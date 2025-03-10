// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
#![allow(unused_imports)]
use crate::kubernetes_api_objects::{api_method::*, common::*};
use crate::kubernetes_cluster::spec::{message::*, reconciler::*};
use crate::pervasive::{map::*, multiset::*, option::*, seq::*, set::*};
use crate::state_machine::action::*;
use crate::state_machine::state_machine::*;
use builtin::*;
use builtin_macros::*;

verus! {

pub struct ControllerState<T> {
    pub req_id: nat,
    pub ongoing_reconciles: Map<ObjectRef, OngoingReconcile<T>>,
    pub scheduled_reconciles: Set<ObjectRef>,
}

pub struct OngoingReconcile<T> {
    pub pending_req_msg: Option<Message>,
    pub local_state: T,
}

pub struct ControllerActionInput {
    pub recv: Option<Message>,
    pub scheduled_cr_key: Option<ObjectRef>,
}

#[is_variant]
pub enum ControllerStep {
    RunScheduledReconcile,
    ContinueReconcile,
    EndReconcile,
}

pub type ControllerStateMachine<T> = StateMachine<ControllerState<T>, ControllerActionInput, ControllerActionInput, Multiset<Message>, ControllerStep>;

pub type ControllerAction<T> = Action<ControllerState<T>, ControllerActionInput, Multiset<Message>>;

pub type ControllerActionResult<T> = ActionResult<ControllerState<T>, Multiset<Message>>;

pub open spec fn controller_req_msg(req: APIRequest, req_id: nat) -> Message {
    form_msg(HostId::CustomController, HostId::KubernetesAPI, MessageContent::APIRequest(req, req_id))
}

pub open spec fn insert_scheduled_reconcile<T>(s: ControllerState<T>, key: ObjectRef) -> ControllerState<T>
    recommends
        key.kind.is_CustomResourceKind(),
{
    ControllerState {
        scheduled_reconciles: s.scheduled_reconciles.insert(key),
        ..s
    }
}

}
