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

//! EDMX to Redfish domain model converter
//!
//! This module handles the conversion from parsed EDMX structures to the Redfish domain model

use super::{
    Capabilities, ComplexTypeData, EnumData, EnumMember, ItemMetadata, NavigationPropertyData,
    Permission, PropertyData, PropertyType, RedfishResource, ResourceReference, SchemaItem,
    Version, VersionedField,
};
use crate::edmx::{
    property::PropertyAttrs,
    schema::{Schema, Type},
    Edmx,
};
use std::collections::HashMap;

impl RedfishResource {
    pub fn from_edmx(edmx: &Edmx) -> Result<Vec<Self>, String> {
        let mut resources = Vec::new();

        let mut base_metadata: HashMap<String, (String, Option<String>, Vec<String>)> =
            HashMap::new();

        for schema in &edmx.data_services.schemas {
            if Self::parse_version_from_namespace(&schema.namespace)?.is_none() {
                let resource_name = schema
                    .namespace
                    .split('.')
                    .next()
                    .unwrap_or(&schema.namespace)
                    .to_string();

                for (_name, schema_type) in &schema.types {
                    if let Type::EntityType(entity_type) = schema_type {
                        if entity_type.name == resource_name {
                            let description = entity_type
                                .annotations
                                .iter()
                                .find(|a| a.term == "OData.Description")
                                .and_then(|a| a.string.clone())
                                .unwrap_or_else(|| format!("Resource {}", resource_name));

                            let long_description = entity_type
                                .annotations
                                .iter()
                                .find(|a| a.term == "OData.LongDescription")
                                .and_then(|a| a.string.clone());

                            let mut uris = Vec::new();
                            for annotation in &entity_type.annotations {
                                if annotation.term == "Redfish.Uris" {
                                    if let Some(collection) = &annotation.collection {
                                        uris.extend(collection.strings.clone());
                                    }
                                    break;
                                }
                            }

                            base_metadata.insert(
                                resource_name.clone(),
                                (description, long_description, uris),
                            );
                            break;
                        }
                    }
                }
            }
        }

        let mut resource_map: HashMap<String, RedfishResource> = HashMap::new();

        for schema in &edmx.data_services.schemas {
            if let Some(version) = Self::parse_version_from_namespace(&schema.namespace)? {
                let resource_name = schema
                    .namespace
                    .split('.')
                    .next()
                    .unwrap_or(&schema.namespace)
                    .to_string();

                let resource = resource_map
                    .entry(resource_name.clone())
                    .or_insert_with(|| {
                        let mut new_resource = RedfishResource {
                            metadata: ItemMetadata {
                                name: resource_name.clone(),
                                description: format!("Resource {}", resource_name),
                                long_description: None,
                            },
                            uris: Vec::new(),
                            items: Vec::new(),
                            capabilities: Capabilities {
                                deletable: None,
                                insertable: None,
                                updatable: None,
                            },
                        };

                        if let Some((description, long_description, uris)) =
                            base_metadata.get(&resource_name)
                        {
                            new_resource.metadata.description = description.clone();
                            new_resource.metadata.long_description = long_description.clone();
                            new_resource.uris = uris.clone();
                        }

                        new_resource
                    });

                let version_items = Self::extract_items_from_schema(schema, &version)?;
                resource.items.extend(version_items);
            }
        }

        resources.extend(resource_map.into_values());

        Ok(resources)
    }

    fn extract_items_from_schema(
        schema: &Schema,
        version: &Version,
    ) -> Result<Vec<VersionedField<SchemaItem>>, String> {
        let mut items = Vec::new();

        for (_, schema_type) in &schema.types {
            match schema_type {
                Type::EntityType(entity_type) => {
                    for property in &entity_type.properties {
                        match &property.attrs {
                            PropertyAttrs::StructuralProperty(structural_prop) => {
                                let item = SchemaItem::Property(PropertyData {
                                    metadata: ItemMetadata {
                                        name: property.name.clone(),
                                        description: structural_prop
                                            .annotations
                                            .iter()
                                            .find(|a| a.term == "OData.Description")
                                            .and_then(|a| a.string.clone())
                                            .unwrap_or_else(|| {
                                                format!("Property {}", property.name)
                                            }),
                                        long_description: structural_prop
                                            .annotations
                                            .iter()
                                            .find(|a| a.term == "OData.LongDescription")
                                            .and_then(|a| a.string.clone()),
                                    },
                                    property_type: Self::convert_property_type(
                                        &structural_prop.ptype,
                                    )?,
                                    nullable: structural_prop.nullable.unwrap_or(false),
                                    permissions: Self::convert_permissions(
                                        &structural_prop.annotations,
                                    ),
                                    units: structural_prop
                                        .annotations
                                        .iter()
                                        .find(|a| {
                                            a.term.contains("Measures") && a.term.contains("Unit")
                                        })
                                        .and_then(|a| a.string.clone()),
                                    constraints: None,
                                });
                                items.push(VersionedField {
                                    field: item,
                                    introduced_in: version.clone(),
                                    deprecated_in: None,
                                });
                            }
                            PropertyAttrs::NavigationProperty(nav_prop) => {
                                let item = SchemaItem::NavigationProperty(NavigationPropertyData {
                                    metadata: ItemMetadata {
                                        name: property.name.clone(),
                                        description: nav_prop
                                            .annotations
                                            .iter()
                                            .find(|a| a.term == "OData.Description")
                                            .and_then(|a| a.string.clone())
                                            .unwrap_or_else(|| {
                                                format!("Navigation property {}", property.name)
                                            }),
                                        long_description: nav_prop
                                            .annotations
                                            .iter()
                                            .find(|a| a.term == "OData.LongDescription")
                                            .and_then(|a| a.string.clone()),
                                    },
                                    target_type: ResourceReference::TypeName(
                                        nav_prop.ptype.clone(),
                                    ),
                                    is_collection: nav_prop.ptype.starts_with("Collection("),
                                    nullable: nav_prop.nullable.unwrap_or(false),
                                    permissions: Self::convert_permissions(&nav_prop.annotations),
                                    auto_expand: nav_prop
                                        .annotations
                                        .iter()
                                        .any(|a| a.term.contains("AutoExpand")),
                                    excerpt_copy: None,
                                });
                                items.push(VersionedField {
                                    field: item,
                                    introduced_in: version.clone(),
                                    deprecated_in: None,
                                });
                            }
                        }
                    }
                }
                Type::ComplexType(complex_type) => {
                    let mut properties = Vec::new();
                    let mut navigation_properties = Vec::new();

                    for property in &complex_type.properties {
                        match &property.attrs {
                            PropertyAttrs::StructuralProperty(structural_prop) => {
                                properties.push(PropertyData {
                                    metadata: ItemMetadata {
                                        name: property.name.clone(),
                                        description: structural_prop
                                            .annotations
                                            .iter()
                                            .find(|a| a.term == "OData.Description")
                                            .and_then(|a| a.string.clone())
                                            .unwrap_or_else(|| {
                                                format!("Property {}", property.name)
                                            }),
                                        long_description: structural_prop
                                            .annotations
                                            .iter()
                                            .find(|a| a.term == "OData.LongDescription")
                                            .and_then(|a| a.string.clone()),
                                    },
                                    property_type: Self::convert_property_type(
                                        &structural_prop.ptype,
                                    )?,
                                    nullable: structural_prop.nullable.unwrap_or(false),
                                    permissions: Self::convert_permissions(
                                        &structural_prop.annotations,
                                    ),
                                    units: structural_prop
                                        .annotations
                                        .iter()
                                        .find(|a| {
                                            a.term.contains("Measures") && a.term.contains("Unit")
                                        })
                                        .and_then(|a| a.string.clone()),
                                    constraints: None,
                                });
                            }
                            PropertyAttrs::NavigationProperty(nav_prop) => {
                                navigation_properties.push(NavigationPropertyData {
                                    metadata: ItemMetadata {
                                        name: property.name.clone(),
                                        description: nav_prop
                                            .annotations
                                            .iter()
                                            .find(|a| a.term == "OData.Description")
                                            .and_then(|a| a.string.clone())
                                            .unwrap_or_else(|| {
                                                format!("Navigation property {}", property.name)
                                            }),
                                        long_description: nav_prop
                                            .annotations
                                            .iter()
                                            .find(|a| a.term == "OData.LongDescription")
                                            .and_then(|a| a.string.clone()),
                                    },
                                    target_type: ResourceReference::TypeName(
                                        nav_prop.ptype.clone(),
                                    ),
                                    is_collection: nav_prop.ptype.starts_with("Collection("),
                                    nullable: nav_prop.nullable.unwrap_or(false),
                                    permissions: Self::convert_permissions(&nav_prop.annotations),
                                    auto_expand: nav_prop
                                        .annotations
                                        .iter()
                                        .any(|a| a.term.contains("AutoExpand")),
                                    excerpt_copy: None,
                                });
                            }
                        }
                    }

                    let item = SchemaItem::ComplexType(ComplexTypeData {
                        metadata: ItemMetadata {
                            name: complex_type.name.clone(),
                            description: complex_type
                                .annotations
                                .iter()
                                .find(|a| a.term == "OData.Description")
                                .and_then(|a| a.string.clone())
                                .unwrap_or_else(|| format!("Complex type {}", complex_type.name)),
                            long_description: complex_type
                                .annotations
                                .iter()
                                .find(|a| a.term == "OData.LongDescription")
                                .and_then(|a| a.string.clone()),
                        },
                        base_type: None, // TODO: Need full types support
                        properties,
                        navigation_properties,
                        additional_properties: complex_type.annotations.iter().any(|a| {
                            a.term == "OData.AdditionalProperties" && a.bool_value == Some(true)
                        }),
                    });
                    items.push(VersionedField {
                        field: item,
                        introduced_in: version.clone(),
                        deprecated_in: None,
                    });
                }
                Type::EnumType(enum_type) => {
                    let item = SchemaItem::Enum(EnumData {
                        metadata: ItemMetadata {
                            name: enum_type.name.clone(),
                            description: enum_type
                                .annotations
                                .iter()
                                .find(|a| a.term == "OData.Description")
                                .and_then(|a| a.string.clone())
                                .unwrap_or_else(|| format!("Enum {}", enum_type.name)),
                            long_description: enum_type
                                .annotations
                                .iter()
                                .find(|a| a.term == "OData.LongDescription")
                                .and_then(|a| a.string.clone()),
                        },
                        members: enum_type
                            .members
                            .iter()
                            .map(|member| EnumMember {
                                name: member.name.clone(),
                                description: member
                                    .annotations
                                    .iter()
                                    .find(|a| a.term == "OData.Description")
                                    .and_then(|a| a.string.clone()),
                            })
                            .collect(),
                    });
                    items.push(VersionedField {
                        field: item,
                        introduced_in: version.clone(),
                        deprecated_in: None,
                    });
                }
                _ => {}
            }
        }

        Ok(items)
    }

    fn parse_version_from_namespace(namespace: &str) -> Result<Option<Version>, String> {
        if !namespace.contains('.') {
            return Ok(None);
        }

        let version_part = namespace.split('.').nth(1);

        if let Some(version_str) = version_part {
            if version_str.starts_with('v') {
                let version_numbers: Vec<&str> = version_str[1..].split('_').collect();
                if version_numbers.len() >= 3 {
                    let major = version_numbers[0]
                        .parse()
                        .map_err(|_| format!("Invalid major version: {}", version_numbers[0]))?;
                    let minor = version_numbers[1]
                        .parse()
                        .map_err(|_| format!("Invalid minor version: {}", version_numbers[1]))?;
                    let patch = version_numbers[2]
                        .parse()
                        .map_err(|_| format!("Invalid patch version: {}", version_numbers[2]))?;
                    return Ok(Some(Version {
                        major,
                        minor,
                        patch,
                    }));
                }
            }
        }

        Ok(None)
    }

    fn convert_property_type(property_type: &str) -> Result<PropertyType, String> {
        match property_type {
            "Edm.String" => Ok(PropertyType::String),
            "Edm.Boolean" => Ok(PropertyType::Boolean),
            "Edm.Decimal" => Ok(PropertyType::Decimal),
            "Edm.Int32" => Ok(PropertyType::Int32),
            "Edm.Int64" => Ok(PropertyType::Int64),
            _ if property_type.starts_with("Collection(") => {
                let inner_type = &property_type[11..property_type.len() - 1];
                Ok(PropertyType::Collection(Box::new(
                    Self::convert_property_type(inner_type)?,
                )))
            }
            _ => {
                // TODO: Temporry, need actual types
                Ok(PropertyType::Reference(ResourceReference::TypeName(
                    property_type.to_string(),
                )))
            }
        }
    }

    fn convert_permissions(annotations: &[crate::edmx::Annotation]) -> Permission {
        for annotation in annotations {
            if annotation.term == "OData.Permissions" {
                if let Some(enum_member) = &annotation.enum_member {
                    return match enum_member.as_str() {
                        "OData.Permission/Read" => Permission::Read,
                        "OData.Permission/Write" => Permission::Write,
                        "OData.Permission/ReadWrite" => Permission::ReadWrite,
                        _ => Permission::Read,
                    };
                }
            }
        }
        Permission::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edmx::Edmx;
    use std::fs;

    fn crate_root() -> std::path::PathBuf {
        std::path::Path::new(&env!("CARGO_MANIFEST_DIR")).to_path_buf()
    }

    #[ignore]
    #[test]
    fn test_edmx_to_redfish_conversion() -> Result<(), Box<dyn std::error::Error>> {
        let data = fs::read_to_string(
            crate_root().join("test-data/redfish-schema/CoolantConnector_v1.xml"),
        )?;

        let edmx: Edmx = Edmx::parse(&data).map_err(|e| format!("EDMX parse error: {:?}", e))?;

        let resources = RedfishResource::from_edmx(&edmx)?;

        assert!(
            !resources.is_empty(),
            "Should have converted at least one resource"
        );

        let coolant_connector = resources
            .iter()
            .find(|r| r.metadata.name == "CoolantConnector")
            .expect("Should have found CoolantConnector resource");

        assert_eq!(coolant_connector.metadata.name, "CoolantConnector");
        assert!(
            !coolant_connector.items.is_empty(),
            "Should have schema items"
        );

        assert!(coolant_connector
            .metadata
            .description
            .contains("liquid coolant connector"));
        assert!(coolant_connector.metadata.long_description.is_some());

        assert!(
            !coolant_connector.uris.is_empty(),
            "Should have URI patterns"
        );
        assert!(coolant_connector
            .uris
            .iter()
            .any(|uri| uri.contains("CoolantConnectors")));

        let properties_count = coolant_connector
            .items
            .iter()
            .filter(|item| matches!(item.field, SchemaItem::Property(_)))
            .count();

        let nav_properties_count = coolant_connector
            .items
            .iter()
            .filter(|item| matches!(item.field, SchemaItem::NavigationProperty(_)))
            .count();

        assert!(
            properties_count > 0 || nav_properties_count > 0,
            "Should have at least some properties or navigation properties"
        );

        println!("{coolant_connector:#?}");

        Ok(())
    }
}
