// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
#![allow(unused_imports)]
use crate::pervasive::{map::*, option::*, seq::*, set::*, string::*};
use crate::state_machine::action::*;
use crate::state_machine::state_machine::*;
use crate::temporal_logic::defs::*;
use crate::tla_examples::compound::common::*;
use builtin::*;
use builtin_macros::*;

verus! {

pub struct State {
    pub resources: Map<ResourceKey, ResourceObj>,
}

pub enum Step {
    HandleRequest,
}

pub type KubernetesAPIStateMachine = StateMachine<State, Option<Message>, Option<Message>, Set<Message>, Step>;

pub type KubernetesAPIAction = Action<State, Option<Message>, Set<Message>>;

pub type KubernetesAPIActionResult = ActionResult<State, Set<Message>>;

pub open spec fn update_resources_with(s: State, msg: Message) -> Map<ResourceKey, ResourceObj>
    recommends
        msg.is_CreateRequest() || msg.is_DeleteRequest(),
{
    if msg.is_CreateRequest() {
        if s.resources.dom().contains(msg.get_CreateRequest_0().obj.key) {
            s.resources
        } else {
            s.resources.insert(msg.get_CreateRequest_0().obj.key, msg.get_CreateRequest_0().obj)
        }
    } else {
        if !s.resources.dom().contains(msg.get_DeleteRequest_0().key) {
            s.resources
        } else {
            s.resources.remove(msg.get_DeleteRequest_0().key)
        }
    }
}

pub open spec fn outcome_messages(s: State, msg: Message) -> Set<Message>
    recommends
        msg.is_CreateRequest() || msg.is_DeleteRequest(),
{
    if msg.is_CreateRequest() {
        if msg.get_CreateRequest_0().obj.key.kind.is_StatefulSetKind() {
            set![
                create_resp_msg(msg.get_CreateRequest_0().obj.key),
                create_req_msg(ResourceKey{name: msg.get_CreateRequest_0().obj.key.name + pod_suffix(), kind: ResourceKind::PodKind}),
                create_req_msg(ResourceKey{name: msg.get_CreateRequest_0().obj.key.name + vol_suffix(), kind: ResourceKind::VolumeKind})
            ]
        } else {
            set![create_resp_msg(msg.get_CreateRequest_0().obj.key)]
        }
    } else {
        if msg.get_DeleteRequest_0().key.kind.is_StatefulSetKind() {
            set![
                delete_resp_msg(msg.get_DeleteRequest_0().key),
                delete_req_msg(ResourceKey{name: msg.get_DeleteRequest_0().key.name + pod_suffix(), kind: ResourceKind::PodKind}),
                delete_req_msg(ResourceKey{name: msg.get_DeleteRequest_0().key.name + vol_suffix(), kind: ResourceKind::VolumeKind})
            ]
        } else {
            set![delete_resp_msg(msg.get_DeleteRequest_0().key)]
        }
    }
}

pub open spec fn handle_request() -> KubernetesAPIAction {
    Action {
        precondition: |recv: Option<Message>, s| {
            &&& recv.is_Some()
            &&& recv.get_Some_0().is_CreateRequest() || recv.get_Some_0().is_DeleteRequest()
        },
        transition: |recv: Option<Message>, s| {
            (State {resources: update_resources_with(s, recv.get_Some_0())}, outcome_messages(s, recv.get_Some_0()))
        },
    }
}

pub open spec fn kubernetes_api() -> KubernetesAPIStateMachine {
    StateMachine {
        init: |s: State| s.resources == Map::<ResourceKey, ResourceObj>::empty(),
        actions: set![handle_request()],
        step_to_action: |step: Step| {
            match step {
                Step::HandleRequest => handle_request(),
            }
        },
        action_input: |step: Step, recv: Option<Message>| {
            recv
        }
    }
}

pub proof fn exists_next_step(action: KubernetesAPIAction, recv: Option<Message>, s: State)
    requires
        kubernetes_api().actions.contains(action),
        (action.precondition)(recv, s),
    ensures
        exists |step| (#[trigger] (kubernetes_api().step_to_action)(step).precondition)(recv, s),
{
    assert(((kubernetes_api().step_to_action)(Step::HandleRequest).precondition)(recv, s));
}

}
