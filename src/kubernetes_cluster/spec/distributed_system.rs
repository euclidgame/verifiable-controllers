// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
#![allow(unused_imports)]
use crate::kubernetes_api_objects::{api_method::*, common::*, object::*};
use crate::kubernetes_cluster::spec::{
    client,
    client::{client, ClientActionInput, ClientState},
    controller::common::{
        insert_scheduled_reconcile, ControllerAction, ControllerActionInput, ControllerState,
        OngoingReconcile,
    },
    controller::state_machine::controller,
    kubernetes_api::common::{KubernetesAPIAction, KubernetesAPIActionInput, KubernetesAPIState},
    kubernetes_api::state_machine::kubernetes_api,
    message::*,
    network,
    network::{multiset_contains_msg, network, NetworkState},
    reconciler::Reconciler,
};
use crate::pervasive::{map::*, option::*, seq::*, set::*};
use crate::state_machine::action::*;
use crate::state_machine::state_machine::*;
use crate::temporal_logic::defs::*;
use builtin::*;
use builtin_macros::*;

verus! {

pub struct State<T> {
    pub kubernetes_api_state: KubernetesAPIState,
    pub controller_state: ControllerState<T>,
    pub client_state: ClientState,
    pub network_state: NetworkState,
}

impl<T> State<T> {
    #[verifier(inline)]
    pub open spec fn message_in_flight(self, msg: Message) -> bool {
        multiset_contains_msg(self.network_state.in_flight, msg)
    }

    pub open spec fn resource_key_exists(self, key: ObjectRef) -> bool {
        self.kubernetes_api_state.resources.dom().contains(key)
    }

    pub open spec fn resource_obj_exists(self, obj: KubernetesObject) -> bool {
        &&& self.kubernetes_api_state.resources.dom().contains(obj.object_ref())
        &&& self.kubernetes_api_state.resources[obj.object_ref()] == obj
    }

    pub open spec fn resource_obj_of(self, key: ObjectRef) -> KubernetesObject
        recommends self.resource_key_exists(key)
    {
        self.kubernetes_api_state.resources[key]
    }

    #[verifier(inline)]
    pub open spec fn reconcile_state_contains(self, key: ObjectRef) -> bool {
        self.controller_state.ongoing_reconciles.dom().contains(key)
    }

    pub open spec fn reconcile_state_of(self, key: ObjectRef) -> OngoingReconcile<T>
        recommends self.reconcile_state_contains(key)
    {
        self.controller_state.ongoing_reconciles[key]
    }

    pub open spec fn reconcile_scheduled_for(self, key: ObjectRef) -> bool {
        self.controller_state.scheduled_reconciles.contains(key)
    }
}

pub open spec fn init<T>(reconciler: Reconciler<T>) -> StatePred<State<T>> {
    |s: State<T>| {
        &&& (kubernetes_api().init)(s.kubernetes_api_state)
        &&& (controller(reconciler).init)(s.controller_state)
        &&& (client().init)(s.client_state)
        &&& (network().init)(s.network_state)
    }
}

pub open spec fn received_msg_destined_for(recv: Option<Message>, host_id: HostId) -> bool {
    if recv.is_Some() {
        recv.get_Some_0().dst == host_id
    } else {
        true
    }
}

pub open spec fn kubernetes_api_next<T>() -> Action<State<T>, KubernetesAPIActionInput, ()> {
    let result = |input: KubernetesAPIActionInput, s: State<T>| {
        let host_result = kubernetes_api().next_result(input, s.kubernetes_api_state);
        let msg_ops = MessageOps {
            recv: input,
            send: host_result.get_Enabled_1(),
        };
        let network_result = network().next_result(msg_ops, s.network_state);

        (host_result, network_result)
    };
    Action {
        precondition: |input: KubernetesAPIActionInput, s: State<T>| {
            &&& received_msg_destined_for(input, HostId::KubernetesAPI)
            &&& result(input, s).0.is_Enabled()
            &&& result(input, s).1.is_Enabled()
        },
        transition: |input: KubernetesAPIActionInput, s: State<T>| {
            (State {
                kubernetes_api_state: result(input, s).0.get_Enabled_0(),
                network_state: result(input, s).1.get_Enabled_0(),
                ..s
            }, ())
        },
    }
}

pub open spec fn controller_next<T>(reconciler: Reconciler<T>) -> Action<State<T>, ControllerActionInput, ()> {
    let result = |input: ControllerActionInput, s: State<T>| {
        let host_result = controller(reconciler).next_result(input, s.controller_state);
        let msg_ops = MessageOps {
            recv: input.recv,
            send: host_result.get_Enabled_1(),
        };
        let network_result = network().next_result(msg_ops, s.network_state);

        (host_result, network_result)
    };
    Action {
        precondition: |input: ControllerActionInput, s: State<T>| {
            &&& received_msg_destined_for(input.recv, HostId::CustomController)
            &&& result(input, s).0.is_Enabled()
            &&& result(input, s).1.is_Enabled()
        },
        transition: |input: ControllerActionInput, s: State<T>| {
            (State {
                controller_state: result(input, s).0.get_Enabled_0(),
                network_state: result(input, s).1.get_Enabled_0(),
                ..s
            }, ())
        },
    }
}

/// This is action checks whether a custom resource exists in the Kubernetes API and if so schedule a controller reconcile for it.
/// It is used to set up the assumption for liveness proof: for a existing cr, the reconcile is infinitely frequently invoked for it.
/// The assumption that cr always exists and the weak fairness assumption on this action allow us to prove reconcile is always eventually scheduled.
///
/// Note this action abstracts away a lot of implementation details in the Kubernetes API and controller runtime framework,
/// such as the list-then-watch pattern.
pub open spec fn schedule_controller_reconcile<T>() -> Action<State<T>, ObjectRef, ()> {
    Action {
        precondition: |input: ObjectRef, s: State<T>| {
            &&& s.resource_key_exists(input)
            &&& input.kind.is_CustomResourceKind()
        },
        transition: |input: ObjectRef, s: State<T>| {
            (State {
                controller_state: insert_scheduled_reconcile(s.controller_state, input),
                ..s
            }, ())
        }
    }
}

pub open spec fn client_next<T>() -> Action<State<T>, ClientActionInput, ()> {
    let result = |input: ClientActionInput, s: State<T>| {
        let host_result = client().next_result(input, s.client_state);
        let msg_ops = MessageOps {
            recv: input.recv,
            send: host_result.get_Enabled_1(),
        };
        let network_result = network().next_result(msg_ops, s.network_state);

        (host_result, network_result)
    };
    Action {
        precondition: |input: ClientActionInput, s: State<T>| {
            &&& received_msg_destined_for(input.recv, HostId::Client)
            &&& result(input, s).0.is_Enabled()
            &&& result(input, s).1.is_Enabled()
        },
        transition: |input: ClientActionInput, s: State<T>| {
            (State {
                client_state: result(input, s).0.get_Enabled_0(),
                network_state: result(input, s).1.get_Enabled_0(),
                ..s
            }, ())
        },
    }
}

// TODO: stutter does not need input
pub open spec fn stutter<T>() -> Action<State<T>, Option<Message>, ()> {
    Action {
        precondition: |input: Option<Message>, s: State<T>| {
            input.is_None()
        },
        transition: |input: Option<Message>, s: State<T>| {
            (s, ())
        },
    }
}

pub enum Step {
    KubernetesAPIStep(KubernetesAPIActionInput),
    ControllerStep(ControllerActionInput),
    ScheduleControllerReconcileStep(ObjectRef),
    ClientStep(ClientActionInput),
    StutterStep(Option<Message>),
}

pub open spec fn next_step<T>(reconciler: Reconciler<T>, s: State<T>, s_prime: State<T>, step: Step) -> bool {
    match step {
        Step::KubernetesAPIStep(input) => kubernetes_api_next().forward(input)(s, s_prime),
        Step::ControllerStep(input) => controller_next(reconciler).forward(input)(s, s_prime),
        Step::ScheduleControllerReconcileStep(input) => schedule_controller_reconcile().forward(input)(s, s_prime),
        Step::ClientStep(input) => client_next().forward(input)(s, s_prime),
        Step::StutterStep(input) => stutter().forward(input)(s, s_prime),
    }
}

/// `next` chooses:
/// * which host to take the next action (`Step`)
/// * whether to deliver a message and which message to deliver (`Option<Message>` in `Step`)
pub open spec fn next<T>(reconciler: Reconciler<T>) -> ActionPred<State<T>> {
    |s: State<T>, s_prime: State<T>| exists |step: Step| next_step(reconciler, s, s_prime, step)
}

/// We install the reconciler to the Kubernetes cluster state machine spec
/// TODO: develop a struct for the compound state machine and make reconciler its member
/// so that we don't have to pass reconciler to init and next in the proof.
pub open spec fn sm_spec<T>(reconciler: Reconciler<T>) -> TempPred<State<T>> {
    lift_state(init(reconciler)).and(sm_partial_spec(reconciler))
}

pub open spec fn sm_partial_spec<T>(reconciler: Reconciler<T>) -> TempPred<State<T>> {
    always(lift_action(next(reconciler)))
    .and(tla_forall(|input| kubernetes_api_next().weak_fairness(input)))
    .and(tla_forall(|input| controller_next(reconciler).weak_fairness(input)))
    .and(tla_forall(|input| schedule_controller_reconcile().weak_fairness(input)))
}

pub open spec fn kubernetes_api_action_pre<T>(action: KubernetesAPIAction, input: KubernetesAPIActionInput) -> StatePred<State<T>> {
    |s: State<T>| {
        let host_result = kubernetes_api().next_action_result(action, input, s.kubernetes_api_state);
        let msg_ops = MessageOps {
            recv: input,
            send: host_result.get_Enabled_1(),
        };
        let network_result = network().next_result(msg_ops, s.network_state);

        &&& received_msg_destined_for(input, HostId::KubernetesAPI)
        &&& host_result.is_Enabled()
        &&& network_result.is_Enabled()
    }
}

pub open spec fn controller_action_pre<T>(reconciler: Reconciler<T>, action: ControllerAction<T>, input: ControllerActionInput) -> StatePred<State<T>> {
    |s: State<T>| {
        let host_result = controller(reconciler).next_action_result(action, input, s.controller_state);
        let msg_ops = MessageOps {
            recv: input.recv,
            send: host_result.get_Enabled_1(),
        };
        let network_result = network().next_result(msg_ops, s.network_state);

        &&& received_msg_destined_for(input.recv, HostId::CustomController)
        &&& host_result.is_Enabled()
        &&& network_result.is_Enabled()
    }
}

}
