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

use crate::Expect;
use nv_redfish_core::action::ActionTarget;
use nv_redfish_core::http::ExpandQuery;
use nv_redfish_core::ActionError;
use nv_redfish_core::Bmc as NvRedfishBmc;
use nv_redfish_core::Empty;
use nv_redfish_core::Expandable;
use nv_redfish_core::ODataETag;
use nv_redfish_core::ODataId;
use serde::Serialize;
use serde_json::Error as JsonError;
use std::collections::VecDeque;
use std::error::Error as StdError;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

#[derive(Debug)]
pub enum Error {
    NotSupported,
    MutexLock(String),
    NothingIsExpected,
    BadResponseJson(JsonError),
    UnexpectedGet(ODataId, Expect),
    UnexpectedExpand(ODataId, Expect),
    UnexpectedUpdate(ODataId, String, Expect),
    UnexpectedCreate(ODataId, String, Expect),
    UnexpectedAction(ActionTarget, String, Expect),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::NotSupported => write!(f, "not supported"),
            Self::MutexLock(err) => write!(f, "lock error: {err}"),
            Self::NothingIsExpected => {
                write!(f, "nothing is expected to happen but something happened")
            }
            Self::BadResponseJson(err) => write!(f, "bad json response: {err}"),
            Self::UnexpectedGet(id, expected) => {
                write!(f, "unexpected get: {id}; expected: {expected:?}")
            }
            Self::UnexpectedExpand(id, expected) => {
                write!(f, "unexpected expand: {id}; expected: {expected:?}")
            }
            Self::UnexpectedUpdate(id, json, expected) => {
                write!(
                    f,
                    "unexpected update: {id}; json: {json} expected: {expected:?}"
                )
            }
            Self::UnexpectedCreate(id, json, expected) => {
                write!(
                    f,
                    "unexpected create: {id}; json: {json} expected: {expected:?}"
                )
            }
            Self::UnexpectedAction(id, json, expected) => {
                write!(
                    f,
                    "unexpected action: {id}; json: {json} expected: {expected:?}"
                )
            }
        }
    }
}

impl StdError for Error {}

impl Error {
    pub fn mutex_lock<T>(err: PoisonError<T>) -> Self {
        Self::MutexLock(err.to_string())
    }
}

#[derive(Default)]
pub struct Bmc {
    expect: Mutex<VecDeque<Expect>>,
}

impl Bmc {
    pub fn expect(&self, exp: Expect) {
        let expect: &mut VecDeque<Expect> = &mut self.expect.lock().expect("not poisoned");
        expect.clear();
        expect.push_front(exp);
    }

    pub fn debug_expect(&self) {
        let expect: &VecDeque<Expect> = &self.expect.lock().expect("not poisoned");
        println!("Expectations (total: {})", expect.len());
        for v in expect {
            println!("{v:#?}");
        }
    }
}

impl NvRedfishBmc for Bmc {
    type Error = Error;

    async fn expand<T>(&self, in_id: &ODataId, _query: ExpandQuery) -> Result<Arc<T>, Error>
    where
        T: Expandable,
    {
        let expect = self
            .expect
            .lock()
            .map_err(Error::mutex_lock)?
            .pop_front()
            .ok_or(Error::NothingIsExpected)?;
        match expect {
            Expect::Expand { id, response } if id == *in_id => {
                let result: T = serde_json::from_value(response).map_err(Error::BadResponseJson)?;
                Ok(Arc::new(result))
            }
            _ => Err(Error::UnexpectedExpand(in_id.clone(), expect)),
        }
    }

    async fn get<T: nv_redfish_core::EntityTypeRef + Sized + for<'a> serde::Deserialize<'a>>(
        &self,
        in_id: &ODataId,
    ) -> Result<Arc<T>, Self::Error> {
        let expect = self
            .expect
            .lock()
            .map_err(Error::mutex_lock)?
            .pop_front()
            .ok_or(Error::NothingIsExpected)?;
        match expect {
            Expect::Get { id, response } if id == *in_id => {
                let result: T = serde_json::from_value(response).map_err(Error::BadResponseJson)?;
                Ok(Arc::new(result))
            }
            _ => Err(Error::UnexpectedGet(in_id.clone(), expect)),
        }
    }

    async fn update<
        V: Sync + Send + Serialize,
        R: Sync + Send + Sized + for<'a> serde::Deserialize<'a>,
    >(
        &self,
        in_id: &ODataId,
        _etag: Option<&ODataETag>,
        update: &V,
    ) -> Result<R, Self::Error> {
        let expect = self
            .expect
            .lock()
            .map_err(Error::mutex_lock)?
            .pop_front()
            .ok_or(Error::NothingIsExpected)?;
        let in_request = serde_json::to_value(update).expect("json serializable");
        match expect {
            Expect::Update {
                id,
                request,
                response,
            } if id == *in_id && request == in_request => {
                let result: R = serde_json::from_value(response).map_err(Error::BadResponseJson)?;
                Ok(result)
            }
            _ => Err(Error::UnexpectedUpdate(
                in_id.clone(),
                in_request.to_string(),
                expect,
            )),
        }
    }

    async fn create<
        V: Sync + Send + Serialize,
        R: Sync + Send + Sized + for<'a> serde::Deserialize<'a>,
    >(
        &self,
        in_id: &ODataId,
        create: &V,
    ) -> Result<R, Self::Error> {
        let expect = self
            .expect
            .lock()
            .map_err(Error::mutex_lock)?
            .pop_front()
            .ok_or(Error::NothingIsExpected)?;
        let in_request = serde_json::to_value(create).expect("json serializable");
        match expect {
            Expect::Create {
                id,
                request,
                response,
            } if id == *in_id && request == in_request => {
                let result: R = serde_json::from_value(response).map_err(Error::BadResponseJson)?;
                Ok(result)
            }
            _ => Err(Error::UnexpectedCreate(
                in_id.clone(),
                in_request.to_string(),
                expect,
            )),
        }
    }

    async fn delete(&self, _id: &ODataId) -> Result<Empty, Self::Error> {
        todo!("unimplimented")
    }

    async fn action<
        T: Send + Sync + serde::Serialize,
        R: Send + Sync + Sized + for<'a> serde::Deserialize<'a>,
    >(
        &self,
        action: &nv_redfish_core::Action<T, R>,
        params: &T,
    ) -> Result<R, Self::Error> {
        let expect = self
            .expect
            .lock()
            .map_err(Error::mutex_lock)?
            .pop_front()
            .ok_or(Error::NothingIsExpected)?;
        let in_request = serde_json::to_value(params).expect("json serializable");
        match expect {
            Expect::Action {
                target,
                request,
                response,
            } if target == action.target && request == in_request => {
                let result: R = serde_json::from_value(response).map_err(Error::BadResponseJson)?;
                Ok(result)
            }
            _ => Err(Error::UnexpectedAction(
                action.target.clone(),
                in_request.to_string(),
                expect,
            )),
        }
    }
}

impl ActionError for Error {
    fn not_supported() -> Self {
        Error::NotSupported
    }
}
