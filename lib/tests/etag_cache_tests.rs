// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[cfg(feature = "reqwest")]
mod cache_integration_tests {
    use nv_redfish::{
        Bmc, EntityTypeRef, ODataETag, ODataId,
        bmc::BmcCredentials,
        http::{BmcReqwestError, HttpBmc, ReqwestClient},
    };
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use url::Url;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestResource {
        #[serde(rename = "@odata.id")]
        id: ODataId,
        #[serde(rename = "@odata.etag")]
        etag: Option<ODataETag>,
        name: String,
        value: i32,
    }

    impl EntityTypeRef for TestResource {
        fn id(&self) -> &ODataId {
            &self.id
        }

        fn etag(&self) -> Option<&ODataETag> {
            self.etag.as_ref()
        }
    }

    fn create_odata_id(s: &str) -> ODataId {
        ODataId::from(s.to_string())
    }

    fn create_odata_etag(s: &str) -> ODataETag {
        ODataETag::from(s.to_string())
    }

    #[tokio::test]
    async fn test_initial_request_caches_resource() {
        let mock_server = MockServer::start().await;
        let resource_path = "/redfish/v1/chassis/1";
        let etag_value = "W/\"abc123\"";

        let test_resource = TestResource {
            id: create_odata_id(resource_path),
            etag: Some(create_odata_etag(etag_value)),
            name: "Test Chassis".to_string(),
            value: 100,
        };

        Mock::given(method("GET"))
            .and(path(resource_path))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&test_resource)
                    .insert_header("etag", etag_value),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = ReqwestClient::new().unwrap();
        let credentials = BmcCredentials::new("root".to_string(), "password".to_string());
        let bmc = HttpBmc::new(client, Url::parse(&mock_server.uri()).unwrap(), credentials);

        let resource_id = create_odata_id(resource_path);
        let result = bmc.get::<TestResource>(&resource_id).await;

        assert!(result.is_ok());
        let retrieved = result.unwrap();
        assert_eq!(retrieved.name, "Test Chassis");
        assert_eq!(retrieved.value, 100);
        assert_eq!(retrieved.etag.as_ref().unwrap().to_string(), etag_value);
    }

    #[tokio::test]
    async fn test_304_not_modified_serves_from_cache() {
        let mock_server = MockServer::start().await;
        let resource_path = "/redfish/v1/managers/1";
        let etag_value = "W/\"def456\"";

        let test_resource = TestResource {
            id: create_odata_id(resource_path),
            etag: Some(create_odata_etag(etag_value)),
            name: "Test Manager".to_string(),
            value: 200,
        };

        Mock::given(method("GET"))
            .and(path(resource_path))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&test_resource)
                    .insert_header("etag", etag_value),
            )
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path(resource_path))
            .and(header("if-none-match", etag_value))
            .respond_with(ResponseTemplate::new(304))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = ReqwestClient::new().unwrap();
        let credentials = BmcCredentials::new("root".to_string(), "password".to_string());
        let bmc = HttpBmc::new(client, Url::parse(&mock_server.uri()).unwrap(), credentials);

        let resource_id = create_odata_id(resource_path);

        let result1 = bmc.get::<TestResource>(&resource_id).await;
        assert!(result1.is_ok());
        let retrieved1 = result1.unwrap();
        assert_eq!(retrieved1.name, "Test Manager");

        let result2 = bmc.get::<TestResource>(&resource_id).await;
        assert!(result2.is_ok());
        let retrieved2 = result2.unwrap();

        assert_eq!(retrieved1.name, retrieved2.name);
        assert_eq!(retrieved1.value, retrieved2.value);

        assert!(Arc::ptr_eq(&retrieved1, &retrieved2));
    }

    #[tokio::test]
    async fn test_etag_changed_updates_cache() {
        let mock_server = MockServer::start().await;
        let resource_path = "/redfish/v1/systems/1";
        let old_etag = "W/\"old123\"";
        let new_etag = "W/\"new456\"";

        let old_resource = TestResource {
            id: create_odata_id(resource_path),
            etag: Some(create_odata_etag(old_etag)),
            name: "Old System".to_string(),
            value: 1,
        };

        let new_resource = TestResource {
            id: create_odata_id(resource_path),
            etag: Some(create_odata_etag(new_etag)),
            name: "Updated System".to_string(),
            value: 2,
        };

        Mock::given(method("GET"))
            .and(path(resource_path))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&old_resource)
                    .insert_header("etag", old_etag),
            )
            .up_to_n_times(1)
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path(resource_path))
            .and(header("if-none-match", old_etag))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&new_resource)
                    .insert_header("etag", new_etag),
            )
            .up_to_n_times(1)
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path(resource_path))
            .and(header("if-none-match", new_etag))
            .respond_with(ResponseTemplate::new(304))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = ReqwestClient::new().unwrap();
        let credentials = BmcCredentials::new("root".to_string(), "password".to_string());
        let bmc = HttpBmc::new(client, Url::parse(&mock_server.uri()).unwrap(), credentials);

        let resource_id = create_odata_id(resource_path);

        let result1 = bmc.get::<TestResource>(&resource_id).await;
        assert!(result1.is_ok());
        let retrieved1 = result1.unwrap();
        assert_eq!(retrieved1.name, "Old System");
        assert_eq!(retrieved1.value, 1);

        let result2 = bmc.get::<TestResource>(&resource_id).await;
        assert!(result2.is_ok());
        let retrieved2 = result2.unwrap();
        assert_eq!(retrieved2.name, "Updated System");
        assert_eq!(retrieved2.value, 2);

        assert!(!Arc::ptr_eq(&retrieved1, &retrieved2));

        let result3 = bmc.get::<TestResource>(&resource_id).await;
        assert!(result3.is_ok());
        let retrieved3 = result3.unwrap();
        assert_eq!(retrieved3.name, "Updated System");
        assert_eq!(retrieved3.value, 2);
        assert!(Arc::ptr_eq(&retrieved2, &retrieved3));
    }

    #[tokio::test]
    async fn test_cache_miss_error() {
        let mock_server = MockServer::start().await;
        let resource_path = "/redfish/v1/nonexistent";

        Mock::given(method("GET"))
            .and(path(resource_path))
            .respond_with(ResponseTemplate::new(304))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = ReqwestClient::new().unwrap();
        let credentials = BmcCredentials::new("root".to_string(), "password".to_string());
        let bmc = HttpBmc::new(client, Url::parse(&mock_server.uri()).unwrap(), credentials);

        let resource_id = create_odata_id(resource_path);
        let result = bmc.get::<TestResource>(&resource_id).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, BmcReqwestError::CacheMiss));
    }
}
