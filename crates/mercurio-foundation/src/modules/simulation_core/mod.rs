use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationClockConfig {
    pub max_time_s: f64,
    pub fixed_step_s: f64,
    pub sample_interval_s: f64,
    pub change_loop_limit: usize,
}

impl Default for SimulationClockConfig {
    fn default() -> Self {
        Self {
            max_time_s: 300.0,
            fixed_step_s: 1.0,
            sample_interval_s: 1.0,
            change_loop_limit: 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConcurrentSimulationScenario {
    pub id: String,
    pub subjects: Vec<ConcurrentSubjectScenario>,
    pub max_steps: usize,
    #[serde(default = "default_step_duration")]
    pub step_duration_s: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock_config: Option<SimulationClockConfig>,
    #[serde(with = "tuple_value_map")]
    pub initial_values: BTreeMap<(String, String), Value>,
    #[serde(default)]
    pub requirements: Vec<SimulationRequirement>,
    #[serde(default)]
    pub objectives: Vec<SimulationObjective>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConcurrentSubjectScenario {
    pub subject_id: String,
    pub machine_id: String,
    #[serde(default)]
    pub initial_state_id: Option<String>,
    #[serde(default)]
    pub events: Vec<SimulationEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationEvent {
    pub id: String,
    pub trigger: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationRequirement {
    pub id: String,
    pub label: String,
    pub expression: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationObjective {
    pub id: String,
    pub label: String,
    pub subject: Option<String>,
    pub feature: Option<String>,
    pub expression: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCaseInfo {
    pub id: String,
    pub label: String,
    pub subject_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationTrace {
    pub scenario_id: String,
    pub subject_id: String,
    pub channels: Vec<SimTraceChannel>,
    pub timeline: Vec<SimTraceEntry>,
    pub status: SimulationStatus,
    #[serde(default)]
    pub requirements: Vec<SimulationRequirement>,
    #[serde(default)]
    pub objectives: Vec<SimulationObjective>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimTraceChannel {
    pub id: String,
    pub unit: Option<String>,
    pub source: SimTraceChannelSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimTraceChannelSource {
    StateMachine,
    RateEffect,
    LookupTable,
    AssignEffect,
}

pub type TraceChannel = SimTraceChannel;
pub type TraceChannelSource = SimTraceChannelSource;
pub type TraceEntry = SimTraceEntry;
pub type TraceEvent = SimTraceEvent;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimTraceEntry {
    pub t: f64,
    pub states: BTreeMap<String, Vec<String>>,
    #[serde(with = "tuple_value_map")]
    pub values: BTreeMap<(String, String), Value>,
    pub events: Vec<SimTraceEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimTraceEvent {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<String>,
    pub transition_id: Option<String>,
    pub trigger: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationStatus {
    Completed,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationModel {
    pub id: String,
    pub machines: Vec<SimulationStateMachine>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_rules: Vec<SimulationDerivedFeatureRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binding_rules: Vec<SimulationBindingRule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationDerivedFeatureRule {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<String>,
    pub feature: String,
    pub expression: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationBindingRule {
    pub id: String,
    pub label: String,
    pub left: SimulationFeatureRef,
    pub right: SimulationFeatureRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationFeatureRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<String>,
    pub feature: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationStateMachine {
    pub id: String,
    pub label: String,
    pub states: Vec<SimulationState>,
    pub transitions: Vec<SimulationTransition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationState {
    pub id: String,
    pub label: String,
    pub parent_state_id: Option<String>,
    pub is_initial: bool,
    pub is_final: bool,
    pub is_orthogonal: bool,
    pub is_history: bool,
    pub entry_behavior: Option<SimulationActionSequence>,
    pub exit_behavior: Option<SimulationActionSequence>,
    pub do_behavior: Option<StateDoBehavior>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationTransition {
    pub id: String,
    pub source: String,
    pub target: String,
    pub trigger: SimulationTrigger,
    pub guard: Option<SimulationGuard>,
    pub effects: Vec<SimulationEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationTriggerKind {
    Event,
    Signal,
    Time,
    After,
    Change,
    Completion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationTrigger {
    pub kind: SimulationTriggerKind,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SimulationGuard {
    ExpressionIr(Value),
    RuntimeFeature(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SimulationEffect {
    Assign(AssignEffect),
    EmitSignal(SignalEffect),
    Log(LogEffect),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssignEffect {
    pub feature: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalEffect {
    pub signal_type: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEffect {
    pub kind: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationActionSequence {
    pub actions: Vec<SimulationActionNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SimulationActionNode {
    Effect(SimulationEffect),
    Decision {
        guard: SimulationGuard,
        then_branch: SimulationActionSequence,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        else_branch: Option<SimulationActionSequence>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StateDoBehavior {
    RateIntegration { rates: Vec<SimulationRate> },
    LookupTable { tables: Vec<SimulationLookupTable> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationRate {
    pub feature: String,
    pub source: SimulationRateSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SimulationRateSource {
    Constant(f64),
    Feature(String),
    ExpressionIr(Value),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationLookupTable {
    pub feature: String,
    pub samples: Vec<SimulationLookupSample>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationLookupSample {
    pub t: f64,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationProfileFinding {
    pub code: String,
    pub message: String,
    pub machine_id: Option<String>,
    pub element_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationProfileError {
    pub findings: Vec<SimulationProfileFinding>,
}

impl fmt::Display for SimulationProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid simulation profile: {} finding(s)",
            self.findings.len()
        )
    }
}

impl std::error::Error for SimulationProfileError {}

#[derive(Debug)]
pub enum CoreSimulationError {
    InvalidProfile(SimulationProfileError),
    MissingStateMachine(String),
    MissingSubject(String),
    MissingInitialState(String),
    InvalidExpression(String),
}

impl fmt::Display for CoreSimulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfile(error) => write!(f, "{error}"),
            Self::MissingStateMachine(id) => write!(f, "missing state machine: {id}"),
            Self::MissingSubject(id) => write!(f, "missing simulation subject: {id}"),
            Self::MissingInitialState(id) => write!(f, "missing initial state: {id}"),
            Self::InvalidExpression(message) => {
                write!(f, "invalid simulation expression: {message}")
            }
        }
    }
}

impl std::error::Error for CoreSimulationError {}

impl From<SimulationProfileError> for CoreSimulationError {
    fn from(error: SimulationProfileError) -> Self {
        Self::InvalidProfile(error)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CoreSubjectRunState<'m> {
    subject_id: String,
    machine: &'m SimulationStateMachine,
    active: Vec<String>,
    event_index: usize,
    events: Vec<SimulationEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CorePendingSignal {
    source_subject_id: String,
    signal_type: String,
    target: Option<String>,
}

pub fn run_concurrent_simulation_model(
    model: &SimulationModel,
    scenario: ConcurrentSimulationScenario,
    clock: SimulationClockConfig,
) -> Result<SimulationTrace, CoreSimulationError> {
    validate_simulation_model(model)?;
    let mut subjects = Vec::<CoreSubjectRunState<'_>>::new();
    for subject in &scenario.subjects {
        if subject.subject_id.is_empty() {
            return Err(CoreSimulationError::MissingSubject(
                subject.subject_id.clone(),
            ));
        }
        let machine = model
            .machines
            .iter()
            .find(|machine| machine.id == subject.machine_id || machine.label == subject.machine_id)
            .ok_or_else(|| CoreSimulationError::MissingStateMachine(subject.machine_id.clone()))?;
        let active = initial_configuration(machine, subject.initial_state_id.as_deref())
            .ok_or_else(|| CoreSimulationError::MissingInitialState(subject.machine_id.clone()))?;
        subjects.push(CoreSubjectRunState {
            subject_id: subject.subject_id.clone(),
            machine,
            active,
            event_index: 0,
            events: subject.events.clone(),
        });
    }

    let mut values = scenario.initial_values.clone();
    let mut pending_signals = VecDeque::<CorePendingSignal>::new();
    let mut history = BTreeMap::<(String, String), String>::new();
    let mut elapsed = BTreeMap::<(String, String), f64>::new();
    let mut t = 0.0;
    let mut step = 0usize;
    let mut status = SimulationStatus::Completed;
    for subject in &subjects {
        for state_id in &subject.active {
            elapsed.insert((subject.subject_id.clone(), state_id.clone()), 0.0);
            apply_state_behavior(
                subject.machine,
                state_id,
                &subject.subject_id,
                &mut values,
                &mut pending_signals,
            );
            apply_state_lookup_tables(
                subject.machine,
                state_id,
                &subject.subject_id,
                &mut values,
                0.0,
            )?;
        }
    }
    propagate_model_values(model, &subjects, &mut values)?;

    let mut timeline = vec![make_core_entry(t, &subjects, &values, Vec::new())];
    let max_steps = scenario.max_steps.max(1);
    while step < max_steps && t <= clock.max_time_s {
        let mut fired = false;
        let mut events = Vec::<SimTraceEvent>::new();

        if fire_immediate_transitions(
            &mut subjects,
            &mut values,
            &mut pending_signals,
            &mut history,
            &mut elapsed,
            &mut step,
            max_steps,
            clock.change_loop_limit,
            &mut events,
        )? {
            propagate_model_values(model, &subjects, &mut values)?;
            fired = true;
        }

        let mut scripted_event_fired = false;
        for subject in subjects.iter_mut() {
            if step >= max_steps || subject.event_index >= subject.events.len() {
                continue;
            }
            let event = subject.events[subject.event_index].clone();
            subject.event_index += 1;
            let Some(transition) = select_transition(
                subject.machine,
                &subject.active,
                SimulationTriggerKind::Event,
                &event.trigger,
                &subject.subject_id,
                &values,
            )
            .cloned() else {
                events.push(SimTraceEvent {
                    kind: "event.dropped".to_string(),
                    subject_id: Some(subject.subject_id.clone()),
                    transition_id: None,
                    trigger: Some(event.trigger),
                    reason: Some("no enabled transition matched event trigger".to_string()),
                });
                status = SimulationStatus::Blocked;
                fired = true;
                continue;
            };
            step += 1;
            let before = subject.active.clone();
            apply_effects(
                &transition.effects,
                &subject.subject_id,
                &mut values,
                &mut pending_signals,
            );
            subject.active = apply_state_change(
                subject.machine,
                &subject.subject_id,
                &before,
                &transition.source,
                &transition.target,
                &mut values,
                &mut pending_signals,
                &mut history,
                &mut elapsed,
            )?;
            events.push(SimTraceEvent {
                kind: "transition".to_string(),
                subject_id: Some(subject.subject_id.clone()),
                transition_id: Some(transition.id),
                trigger: Some(event.trigger),
                reason: None,
            });
            scripted_event_fired = true;
            fired = true;
        }
        if scripted_event_fired {
            propagate_model_values(model, &subjects, &mut values)?;
        }

        if fire_immediate_transitions(
            &mut subjects,
            &mut values,
            &mut pending_signals,
            &mut history,
            &mut elapsed,
            &mut step,
            max_steps,
            clock.change_loop_limit,
            &mut events,
        )? {
            propagate_model_values(model, &subjects, &mut values)?;
            fired = true;
        }

        if !fired && step < max_steps {
            let next_after = next_after_duration(&subjects, &elapsed, &values);
            let next_change = next_change_crossing_duration(&subjects, &values)?;
            let fixed_step = clock.fixed_step_s.max(0.0);
            let mut duration = [Some(fixed_step), next_after, next_change]
                .into_iter()
                .flatten()
                .filter(|duration| duration.is_finite() && *duration >= 0.0)
                .min_by(|left, right| left.total_cmp(right))
                .unwrap_or(fixed_step);
            if fixed_step > 0.0 {
                duration = duration.min(fixed_step);
            }
            if duration <= 0.0 {
                duration = fixed_step;
            }
            if duration > 0.0 && t + duration <= clock.max_time_s {
                integrate_active_state_behaviors(
                    &subjects,
                    &mut values,
                    &mut elapsed,
                    duration,
                    clock.sample_interval_s,
                    &mut timeline,
                    t,
                )?;
                propagate_model_values(model, &subjects, &mut values)?;
                t += duration;
                step += 1;
                fired = true;
                fire_after_transitions(
                    &mut subjects,
                    &mut values,
                    &mut pending_signals,
                    &mut history,
                    &mut elapsed,
                    &mut step,
                    max_steps,
                    &mut events,
                )?;
                propagate_model_values(model, &subjects, &mut values)?;
                fire_immediate_transitions(
                    &mut subjects,
                    &mut values,
                    &mut pending_signals,
                    &mut history,
                    &mut elapsed,
                    &mut step,
                    max_steps,
                    clock.change_loop_limit,
                    &mut events,
                )?;
                propagate_model_values(model, &subjects, &mut values)?;
            }
        }

        if fired {
            timeline.push(make_core_entry(t, &subjects, &values, events));
        } else {
            break;
        }
    }

    let primary_subject_id = scenario
        .subjects
        .first()
        .map(|subject| subject.subject_id.clone())
        .unwrap_or_default();
    let generated_channels = generated_continuous_channels(&subjects);
    let channels = values
        .keys()
        .map(|(subject, feature)| SimTraceChannel {
            id: format!("{subject}.{feature}"),
            unit: None,
            source: generated_channels
                .get(&(subject.clone(), feature.clone()))
                .cloned()
                .unwrap_or(SimTraceChannelSource::AssignEffect),
        })
        .collect();
    Ok(SimulationTrace {
        scenario_id: scenario.id,
        subject_id: primary_subject_id,
        channels,
        timeline,
        status,
        requirements: scenario.requirements,
        objectives: scenario.objectives,
    })
}

fn make_core_entry(
    t: f64,
    subjects: &[CoreSubjectRunState<'_>],
    values: &BTreeMap<(String, String), Value>,
    events: Vec<SimTraceEvent>,
) -> SimTraceEntry {
    SimTraceEntry {
        t,
        states: subjects
            .iter()
            .map(|subject| (subject.subject_id.clone(), subject.active.clone()))
            .collect(),
        values: values.clone(),
        events,
    }
}

fn propagate_model_values(
    model: &SimulationModel,
    subjects: &[CoreSubjectRunState<'_>],
    values: &mut BTreeMap<(String, String), Value>,
) -> Result<(), CoreSimulationError> {
    propagate_derived_values(&model.derived_rules, subjects, values)?;
    propagate_binding_values(&model.binding_rules, subjects, values)
}

fn propagate_derived_values(
    rules: &[SimulationDerivedFeatureRule],
    subjects: &[CoreSubjectRunState<'_>],
    values: &mut BTreeMap<(String, String), Value>,
) -> Result<(), CoreSimulationError> {
    if rules.is_empty() {
        return Ok(());
    }
    for rule_index in derived_rule_order(rules).map_err(CoreSimulationError::InvalidProfile)? {
        let rule = &rules[rule_index];
        for subject in subjects {
            if let Some(rule_subject_id) = &rule.subject_id
                && rule_subject_id != &subject.subject_id
            {
                continue;
            }
            match eval_value(&rule.expression, &subject.subject_id, values) {
                Ok(Value::Null) => continue,
                Ok(value) => {
                    values.insert((subject.subject_id.clone(), rule.feature.clone()), value);
                }
                Err(error) if rule.subject_id.is_none() => {
                    if matches!(error, CoreSimulationError::InvalidExpression(_)) {
                        continue;
                    }
                    return Err(error);
                }
                Err(error) => return Err(error),
            }
        }
    }
    Ok(())
}

fn propagate_binding_values(
    rules: &[SimulationBindingRule],
    subjects: &[CoreSubjectRunState<'_>],
    values: &mut BTreeMap<(String, String), Value>,
) -> Result<(), CoreSimulationError> {
    if rules.is_empty() {
        return Ok(());
    }
    for _ in 0..rules.len().max(1) {
        let mut changed = false;
        for rule in rules {
            let left_keys = binding_endpoint_keys(&rule.left, subjects);
            let right_keys = binding_endpoint_keys(&rule.right, subjects);
            if left_keys.is_empty() || right_keys.is_empty() {
                continue;
            }
            for left_key in &left_keys {
                for right_key in &right_keys {
                    changed |= propagate_binding_pair(&rule.id, left_key, right_key, values)?;
                }
            }
        }
        if !changed {
            return Ok(());
        }
    }
    Ok(())
}

fn binding_endpoint_keys(
    endpoint: &SimulationFeatureRef,
    subjects: &[CoreSubjectRunState<'_>],
) -> Vec<(String, String)> {
    if let Some(subject_id) = &endpoint.subject_id {
        return vec![(subject_id.clone(), endpoint.feature.clone())];
    }
    subjects
        .iter()
        .map(|subject| (subject.subject_id.clone(), endpoint.feature.clone()))
        .collect()
}

fn propagate_binding_pair(
    rule_id: &str,
    left_key: &(String, String),
    right_key: &(String, String),
    values: &mut BTreeMap<(String, String), Value>,
) -> Result<bool, CoreSimulationError> {
    let left = values.get(left_key).cloned();
    let right = values.get(right_key).cloned();
    match (left, right) {
        (Some(left), Some(right)) if left != right => {
            Err(CoreSimulationError::InvalidExpression(format!(
                "binding rule `{rule_id}` has conflicting values for {}.{} and {}.{}",
                left_key.0, left_key.1, right_key.0, right_key.1
            )))
        }
        (Some(left), None) => {
            values.insert(right_key.clone(), left);
            Ok(true)
        }
        (None, Some(right)) => {
            values.insert(left_key.clone(), right);
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn derived_rule_order(
    rules: &[SimulationDerivedFeatureRule],
) -> Result<Vec<usize>, SimulationProfileError> {
    let mut outgoing = vec![BTreeSet::<usize>::new(); rules.len()];
    let mut incoming_count = vec![0usize; rules.len()];
    for (dependent_index, dependent) in rules.iter().enumerate() {
        for path in expression_paths(&dependent.expression) {
            for (dependency_index, dependency) in rules.iter().enumerate() {
                if !derived_rule_subjects_overlap(dependent, dependency) {
                    continue;
                }
                if path == dependency.feature || path.ends_with(&format!(".{}", dependency.feature))
                {
                    if outgoing[dependency_index].insert(dependent_index) {
                        incoming_count[dependent_index] += 1;
                    }
                }
            }
        }
    }

    let mut ready = incoming_count
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count == 0).then_some(index))
        .collect::<Vec<_>>();
    let mut ordered = Vec::new();
    while let Some(index) = ready.pop() {
        ordered.push(index);
        for dependent in outgoing[index].clone() {
            incoming_count[dependent] -= 1;
            if incoming_count[dependent] == 0 {
                ready.push(dependent);
            }
        }
    }
    if ordered.len() == rules.len() {
        return Ok(ordered);
    }
    Err(SimulationProfileError {
        findings: vec![finding(
            "derived_rule.cycle",
            "Derived feature rules must be acyclic.",
            None,
            None,
        )],
    })
}

fn derived_rule_subjects_overlap(
    left: &SimulationDerivedFeatureRule,
    right: &SimulationDerivedFeatureRule,
) -> bool {
    match (&left.subject_id, &right.subject_id) {
        (Some(left), Some(right)) => left == right,
        _ => true,
    }
}

fn expression_paths(expression: &Value) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    collect_expression_paths(expression, &mut paths);
    paths
}

fn collect_expression_paths(expression: &Value, paths: &mut BTreeSet<String>) {
    let Some(object) = expression.as_object() else {
        return;
    };
    if object.get("kind").and_then(Value::as_str) == Some("path")
        && let Some(path) = expression_path(expression)
    {
        paths.insert(path);
    }
    for value in object.values() {
        match value {
            Value::Array(items) => {
                for item in items {
                    collect_expression_paths(item, paths);
                }
            }
            Value::Object(_) => collect_expression_paths(value, paths),
            _ => {}
        }
    }
}

fn generated_continuous_channels(
    subjects: &[CoreSubjectRunState<'_>],
) -> BTreeMap<(String, String), SimTraceChannelSource> {
    let mut channels = BTreeMap::new();
    for subject in subjects {
        for state in &subject.machine.states {
            match &state.do_behavior {
                Some(StateDoBehavior::RateIntegration { rates }) => {
                    for rate in rates {
                        channels.insert(
                            (subject.subject_id.clone(), rate.feature.clone()),
                            SimTraceChannelSource::RateEffect,
                        );
                    }
                }
                Some(StateDoBehavior::LookupTable { tables }) => {
                    for table in tables {
                        channels
                            .entry((subject.subject_id.clone(), table.feature.clone()))
                            .or_insert(SimTraceChannelSource::LookupTable);
                    }
                }
                None => {}
            }
        }
    }
    channels
}

fn fire_immediate_transitions(
    subjects: &mut [CoreSubjectRunState<'_>],
    values: &mut BTreeMap<(String, String), Value>,
    pending_signals: &mut VecDeque<CorePendingSignal>,
    history: &mut BTreeMap<(String, String), String>,
    elapsed: &mut BTreeMap<(String, String), f64>,
    step: &mut usize,
    max_steps: usize,
    change_loop_limit: usize,
    events: &mut Vec<SimTraceEvent>,
) -> Result<bool, CoreSimulationError> {
    let mut fired = false;
    if deliver_pending_signals(
        subjects,
        values,
        pending_signals,
        history,
        elapsed,
        step,
        max_steps,
        events,
    )? {
        fired = true;
    }
    for iteration in 0..change_loop_limit {
        let mut loop_fired = false;
        for subject in subjects.iter_mut() {
            if *step >= max_steps {
                break;
            }
            let Some(transition) = select_completion_or_change_transition(
                subject.machine,
                &subject.active,
                &subject.subject_id,
                values,
            )
            .cloned() else {
                continue;
            };
            *step += 1;
            let before = subject.active.clone();
            apply_effects(
                &transition.effects,
                &subject.subject_id,
                values,
                pending_signals,
            );
            subject.active = apply_state_change(
                subject.machine,
                &subject.subject_id,
                &before,
                &transition.source,
                &transition.target,
                values,
                pending_signals,
                history,
                elapsed,
            )?;
            events.push(SimTraceEvent {
                kind: "transition".to_string(),
                subject_id: Some(subject.subject_id.clone()),
                transition_id: Some(transition.id.clone()),
                trigger: Some(match transition.trigger.kind {
                    SimulationTriggerKind::Completion => "completion".to_string(),
                    SimulationTriggerKind::Change => {
                        format!(
                            "change:{}",
                            transition.trigger.value.as_deref().unwrap_or("")
                        )
                    }
                    _ => transition.trigger.value.clone().unwrap_or_default(),
                }),
                reason: None,
            });
            loop_fired = true;
            fired = true;
        }
        let limit_reached_with_pending_transition = loop_fired
            && iteration + 1 == change_loop_limit
            && subjects.iter().any(|subject| {
                select_completion_or_change_transition(
                    subject.machine,
                    &subject.active,
                    &subject.subject_id,
                    values,
                )
                .is_some()
            });
        if limit_reached_with_pending_transition {
            events.push(SimTraceEvent {
                kind: "change.loop.limit".to_string(),
                subject_id: None,
                transition_id: None,
                trigger: None,
                reason: Some(format!(
                    "change transition loop reached configured limit {change_loop_limit}"
                )),
            });
        }
        if !loop_fired {
            break;
        }
    }
    Ok(fired)
}

fn deliver_pending_signals(
    subjects: &mut [CoreSubjectRunState<'_>],
    values: &mut BTreeMap<(String, String), Value>,
    pending_signals: &mut VecDeque<CorePendingSignal>,
    history: &mut BTreeMap<(String, String), String>,
    elapsed: &mut BTreeMap<(String, String), f64>,
    step: &mut usize,
    max_steps: usize,
    events: &mut Vec<SimTraceEvent>,
) -> Result<bool, CoreSimulationError> {
    let mut fired = false;
    let signal_count = pending_signals.len();
    for _ in 0..signal_count {
        if *step >= max_steps {
            break;
        }
        let Some(signal) = pending_signals.pop_front() else {
            break;
        };
        let mut consumed = false;
        for subject in subjects.iter_mut() {
            if *step >= max_steps || !signal_targets_subject(&signal, &subject.subject_id) {
                continue;
            }
            let Some(transition) = select_transition(
                subject.machine,
                &subject.active,
                SimulationTriggerKind::Signal,
                &signal.signal_type,
                &subject.subject_id,
                values,
            )
            .cloned() else {
                continue;
            };
            *step += 1;
            let before = subject.active.clone();
            apply_effects(
                &transition.effects,
                &subject.subject_id,
                values,
                pending_signals,
            );
            subject.active = apply_state_change(
                subject.machine,
                &subject.subject_id,
                &before,
                &transition.source,
                &transition.target,
                values,
                pending_signals,
                history,
                elapsed,
            )?;
            events.push(SimTraceEvent {
                kind: "transition".to_string(),
                subject_id: Some(subject.subject_id.clone()),
                transition_id: Some(transition.id.clone()),
                trigger: Some(format!(
                    "signal:{}:{}",
                    signal.source_subject_id, signal.signal_type
                )),
                reason: None,
            });
            consumed = true;
            fired = true;
        }
        if !consumed {
            pending_signals.push_back(signal);
        }
    }
    Ok(fired)
}

fn signal_targets_subject(signal: &CorePendingSignal, subject_id: &str) -> bool {
    match signal.target.as_deref() {
        Some(target) => target == subject_id,
        None => true,
    }
}

fn select_transition<'a>(
    machine: &'a SimulationStateMachine,
    active: &[String],
    kind: SimulationTriggerKind,
    trigger: &str,
    subject_id: &str,
    values: &BTreeMap<(String, String), Value>,
) -> Option<&'a SimulationTransition> {
    active.iter().rev().find_map(|state_id| {
        machine.transitions.iter().find(|transition| {
            transition.source == *state_id
                && transition.trigger.kind == kind
                && transition.trigger.value.as_deref() == Some(trigger)
                && guard_allows(&transition.guard, subject_id, values)
        })
    })
}

fn select_completion_or_change_transition<'a>(
    machine: &'a SimulationStateMachine,
    active: &[String],
    subject_id: &str,
    values: &BTreeMap<(String, String), Value>,
) -> Option<&'a SimulationTransition> {
    active.iter().rev().find_map(|state_id| {
        machine.transitions.iter().find(|transition| {
            transition.source == *state_id
                && matches!(
                    transition.trigger.kind,
                    SimulationTriggerKind::Completion | SimulationTriggerKind::Change
                )
                && guard_allows(&transition.guard, subject_id, values)
                && transition.trigger.value.as_deref().is_none_or(|value| {
                    value.is_empty() || bool_expression_string(value, subject_id, values)
                })
        })
    })
}

fn apply_state_change(
    machine: &SimulationStateMachine,
    subject_id: &str,
    before: &[String],
    source_state_id: &str,
    target_state_id: &str,
    values: &mut BTreeMap<(String, String), Value>,
    pending_signals: &mut VecDeque<CorePendingSignal>,
    history: &mut BTreeMap<(String, String), String>,
    elapsed: &mut BTreeMap<(String, String), f64>,
) -> Result<Vec<String>, CoreSimulationError> {
    let resolved_target = resolve_history_target(machine, subject_id, target_state_id, history)
        .unwrap_or_else(|| target_state_id.to_string());
    let target_configuration = initial_configuration(machine, Some(&resolved_target))
        .ok_or_else(|| CoreSimulationError::MissingInitialState(resolved_target.clone()))?;
    let source_path = ancestor_path(machine, source_state_id)
        .ok_or_else(|| CoreSimulationError::MissingInitialState(source_state_id.to_string()))?;
    let target_path = ancestor_path(machine, &resolved_target)
        .ok_or_else(|| CoreSimulationError::MissingInitialState(resolved_target.clone()))?;
    let common_prefix_len = source_path
        .iter()
        .zip(target_path.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let common_ancestor = common_prefix_len
        .checked_sub(1)
        .and_then(|index| source_path.get(index));
    let exit_states = before
        .iter()
        .filter(|state_id| {
            state_id.as_str() == source_state_id
                || is_descendant_of(machine, state_id, source_state_id)
                || (common_ancestor.is_none_or(|ancestor| state_id.as_str() != ancestor)
                    && source_path.contains(state_id))
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut after = before
        .iter()
        .filter(|state_id| !exit_states.contains(state_id))
        .cloned()
        .collect::<Vec<_>>();
    let entry_states = target_configuration
        .iter()
        .filter(|state_id| !after.contains(state_id))
        .cloned()
        .collect::<Vec<_>>();
    after.extend(entry_states.clone());

    for state_id in exit_states.iter().rev() {
        apply_exit_behavior(machine, state_id, subject_id, values, pending_signals);
        record_shallow_history(machine, subject_id, state_id, history);
    }
    for state_id in &entry_states {
        elapsed.insert((subject_id.to_string(), state_id.clone()), 0.0);
        apply_state_behavior(machine, state_id, subject_id, values, pending_signals);
        apply_state_lookup_tables(machine, state_id, subject_id, values, 0.0)?;
    }
    Ok(after)
}

fn next_after_duration(
    subjects: &[CoreSubjectRunState<'_>],
    elapsed: &BTreeMap<(String, String), f64>,
    values: &BTreeMap<(String, String), Value>,
) -> Option<f64> {
    subjects
        .iter()
        .flat_map(|subject| {
            subject.active.iter().rev().flat_map(move |state_id| {
                subject
                    .machine
                    .transitions
                    .iter()
                    .filter_map(move |transition| {
                        if transition.source != *state_id
                            || !matches!(
                                transition.trigger.kind,
                                SimulationTriggerKind::After | SimulationTriggerKind::Time
                            )
                            || !guard_allows(&transition.guard, &subject.subject_id, values)
                        {
                            return None;
                        }
                        let duration = parse_duration_s(transition.trigger.value.as_deref()?)?;
                        let active_for = elapsed
                            .get(&(subject.subject_id.clone(), state_id.clone()))
                            .copied()
                            .unwrap_or_default();
                        Some((duration - active_for).max(0.0))
                    })
            })
        })
        .min_by(|left, right| left.total_cmp(right))
}

fn fire_after_transitions(
    subjects: &mut [CoreSubjectRunState<'_>],
    values: &mut BTreeMap<(String, String), Value>,
    pending_signals: &mut VecDeque<CorePendingSignal>,
    history: &mut BTreeMap<(String, String), String>,
    elapsed: &mut BTreeMap<(String, String), f64>,
    step: &mut usize,
    max_steps: usize,
    events: &mut Vec<SimTraceEvent>,
) -> Result<bool, CoreSimulationError> {
    let mut fired = false;
    for subject in subjects.iter_mut() {
        if *step >= max_steps {
            break;
        }
        let Some(transition) = subject
            .active
            .iter()
            .rev()
            .find_map(|state_id| {
                subject.machine.transitions.iter().find(|transition| {
                    if transition.source != *state_id
                        || !matches!(
                            transition.trigger.kind,
                            SimulationTriggerKind::After | SimulationTriggerKind::Time
                        )
                        || !guard_allows(&transition.guard, &subject.subject_id, values)
                    {
                        return false;
                    }
                    let Some(duration) = transition
                        .trigger
                        .value
                        .as_deref()
                        .and_then(parse_duration_s)
                    else {
                        return false;
                    };
                    elapsed
                        .get(&(subject.subject_id.clone(), state_id.clone()))
                        .copied()
                        .unwrap_or_default()
                        + f64::EPSILON
                        >= duration
                })
            })
            .cloned()
        else {
            continue;
        };
        *step += 1;
        let before = subject.active.clone();
        apply_effects(
            &transition.effects,
            &subject.subject_id,
            values,
            pending_signals,
        );
        subject.active = apply_state_change(
            subject.machine,
            &subject.subject_id,
            &before,
            &transition.source,
            &transition.target,
            values,
            pending_signals,
            history,
            elapsed,
        )?;
        events.push(SimTraceEvent {
            kind: "transition".to_string(),
            subject_id: Some(subject.subject_id.clone()),
            transition_id: Some(transition.id.clone()),
            trigger: Some(format!(
                "{}:{}",
                match transition.trigger.kind {
                    SimulationTriggerKind::Time => "time",
                    _ => "after",
                },
                transition.trigger.value.as_deref().unwrap_or_default()
            )),
            reason: None,
        });
        fired = true;
    }
    Ok(fired)
}

fn integrate_active_state_behaviors(
    subjects: &[CoreSubjectRunState<'_>],
    values: &mut BTreeMap<(String, String), Value>,
    elapsed: &mut BTreeMap<(String, String), f64>,
    duration: f64,
    sample_interval_s: f64,
    timeline: &mut Vec<SimTraceEntry>,
    start_t: f64,
) -> Result<(), CoreSimulationError> {
    if duration <= 0.0 {
        return Ok(());
    }
    let sample_interval = sample_interval_s.max(0.0);
    let mut remaining = duration;
    let mut cursor_t = start_t;
    while remaining > f64::EPSILON {
        let dt = if sample_interval > 0.0 {
            remaining.min(sample_interval)
        } else {
            remaining
        };
        integrate_active_rates_once(subjects, values, dt)?;
        for subject in subjects {
            for state_id in &subject.active {
                *elapsed
                    .entry((subject.subject_id.clone(), state_id.clone()))
                    .or_default() += dt;
            }
        }
        apply_active_lookup_tables(subjects, elapsed, values)?;
        cursor_t += dt;
        remaining -= dt;
        if sample_interval > 0.0 && remaining > f64::EPSILON {
            timeline.push(make_core_entry(cursor_t, subjects, values, Vec::new()));
        }
    }
    Ok(())
}

fn integrate_active_rates_once(
    subjects: &[CoreSubjectRunState<'_>],
    values: &mut BTreeMap<(String, String), Value>,
    duration: f64,
) -> Result<(), CoreSimulationError> {
    let snapshot = values.clone();
    let active_rates = active_rates(subjects);
    if active_rates.is_empty() {
        return Ok(());
    }

    let k1 = evaluate_rate_vector(&active_rates, &snapshot, &BTreeMap::new())?;
    let half_dt = duration / 2.0;
    let k2 = evaluate_rate_vector(&active_rates, &snapshot, &scaled_increments(&k1, half_dt))?;
    let k3 = evaluate_rate_vector(&active_rates, &snapshot, &scaled_increments(&k2, half_dt))?;
    let k4 = evaluate_rate_vector(&active_rates, &snapshot, &scaled_increments(&k3, duration))?;

    for active_rate in active_rates {
        let current = snapshot
            .get(&active_rate.key)
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let delta = duration
            * (k1.get(&active_rate.key).copied().unwrap_or_default()
                + 2.0 * k2.get(&active_rate.key).copied().unwrap_or_default()
                + 2.0 * k3.get(&active_rate.key).copied().unwrap_or_default()
                + k4.get(&active_rate.key).copied().unwrap_or_default())
            / 6.0;
        values.insert(active_rate.key, Value::from(current + delta));
    }
    Ok(())
}

fn apply_active_lookup_tables(
    subjects: &[CoreSubjectRunState<'_>],
    elapsed: &BTreeMap<(String, String), f64>,
    values: &mut BTreeMap<(String, String), Value>,
) -> Result<(), CoreSimulationError> {
    for subject in subjects {
        for state_id in &subject.active {
            let active_for = elapsed
                .get(&(subject.subject_id.clone(), state_id.clone()))
                .copied()
                .unwrap_or_default();
            apply_state_lookup_tables(
                subject.machine,
                state_id,
                &subject.subject_id,
                values,
                active_for,
            )?;
        }
    }
    Ok(())
}

fn apply_state_lookup_tables(
    machine: &SimulationStateMachine,
    state_id: &str,
    subject_id: &str,
    values: &mut BTreeMap<(String, String), Value>,
    active_for_s: f64,
) -> Result<(), CoreSimulationError> {
    let Some(state) = machine.states.iter().find(|state| state.id == state_id) else {
        return Ok(());
    };
    let Some(StateDoBehavior::LookupTable { tables }) = &state.do_behavior else {
        return Ok(());
    };
    for table in tables {
        if let Some(value) = lookup_table_value(table, active_for_s)? {
            values.insert(
                (subject_id.to_string(), table.feature.clone()),
                Value::from(value),
            );
        }
    }
    Ok(())
}

fn lookup_table_value(
    table: &SimulationLookupTable,
    active_for_s: f64,
) -> Result<Option<f64>, CoreSimulationError> {
    if table.samples.is_empty() {
        return Ok(None);
    }
    if !active_for_s.is_finite() {
        return Err(CoreSimulationError::InvalidExpression(format!(
            "lookup table `{}` received non-finite elapsed time",
            table.feature
        )));
    }
    let mut samples = table.samples.clone();
    samples.sort_by(|left, right| left.t.total_cmp(&right.t));
    for sample in &samples {
        if !sample.t.is_finite() || !sample.value.is_finite() {
            return Err(CoreSimulationError::InvalidExpression(format!(
                "lookup table `{}` contains a non-finite sample",
                table.feature
            )));
        }
    }
    let Some(first) = samples.first() else {
        return Ok(None);
    };
    if active_for_s <= first.t {
        return Ok(Some(first.value));
    }
    let Some(last) = samples.last() else {
        return Ok(None);
    };
    if active_for_s >= last.t {
        return Ok(Some(last.value));
    }
    for window in samples.windows(2) {
        let [left, right] = window else {
            continue;
        };
        if active_for_s < left.t || active_for_s > right.t {
            continue;
        }
        let span = right.t - left.t;
        if span.abs() <= f64::EPSILON {
            return Ok(Some(right.value));
        }
        let fraction = (active_for_s - left.t) / span;
        return Ok(Some(left.value + (right.value - left.value) * fraction));
    }
    Ok(Some(last.value))
}

#[derive(Debug, Clone)]
struct ActiveRate<'a> {
    key: (String, String),
    subject_id: String,
    source: &'a SimulationRateSource,
}

fn active_rates<'model>(subjects: &[CoreSubjectRunState<'model>]) -> Vec<ActiveRate<'model>> {
    let mut active = Vec::new();
    for subject in subjects {
        for state_id in &subject.active {
            let Some(state) = subject
                .machine
                .states
                .iter()
                .find(|state| state.id == *state_id)
            else {
                continue;
            };
            let Some(StateDoBehavior::RateIntegration { rates }) = &state.do_behavior else {
                continue;
            };
            for rate in rates {
                active.push(ActiveRate {
                    key: (subject.subject_id.clone(), rate.feature.clone()),
                    subject_id: subject.subject_id.clone(),
                    source: &rate.source,
                });
            }
        }
    }
    active
}

fn evaluate_rate_vector(
    active_rates: &[ActiveRate<'_>],
    base_values: &BTreeMap<(String, String), Value>,
    increments: &BTreeMap<(String, String), f64>,
) -> Result<BTreeMap<(String, String), f64>, CoreSimulationError> {
    let values = values_with_increments(base_values, increments);
    let mut vector = BTreeMap::new();
    for active_rate in active_rates {
        vector.insert(
            active_rate.key.clone(),
            rate_value(active_rate.source, &active_rate.subject_id, &values)?,
        );
    }
    Ok(vector)
}

fn scaled_increments(
    vector: &BTreeMap<(String, String), f64>,
    duration: f64,
) -> BTreeMap<(String, String), f64> {
    vector
        .iter()
        .map(|(key, value)| (key.clone(), value * duration))
        .collect()
}

fn values_with_increments(
    values: &BTreeMap<(String, String), Value>,
    increments: &BTreeMap<(String, String), f64>,
) -> BTreeMap<(String, String), Value> {
    let mut adjusted = values.clone();
    for (key, increment) in increments {
        let current = values.get(key).and_then(Value::as_f64).unwrap_or_default();
        adjusted.insert(key.clone(), Value::from(current + increment));
    }
    adjusted
}

fn next_change_crossing_duration(
    subjects: &[CoreSubjectRunState<'_>],
    values: &BTreeMap<(String, String), Value>,
) -> Result<Option<f64>, CoreSimulationError> {
    let mut earliest: Option<f64> = None;
    for subject in subjects {
        for state_id in subject.active.iter().rev() {
            for transition in subject.machine.transitions.iter().filter(|transition| {
                transition.source == *state_id
                    && transition.trigger.kind == SimulationTriggerKind::Change
            }) {
                let expression = transition
                    .trigger
                    .value
                    .as_deref()
                    .or_else(|| match &transition.guard {
                        Some(SimulationGuard::RuntimeFeature(feature)) => Some(feature.as_str()),
                        _ => None,
                    });
                let Some(expression) = expression else {
                    continue;
                };
                let Some((feature, op, threshold)) =
                    comparison_against_threshold(expression, &subject.subject_id, values)
                else {
                    continue;
                };
                let Some(current) = resolve_feature_path(&feature, &subject.subject_id, values)
                    .and_then(|value| value.as_f64())
                else {
                    continue;
                };
                if compare_numbers(current, threshold, op) {
                    earliest = Some(0.0);
                    continue;
                }
                let Some(rate) = active_rate_for_feature(subject, state_id, &feature, values)?
                else {
                    continue;
                };
                if rate.abs() <= f64::EPSILON {
                    continue;
                }
                let duration = (threshold - current) / rate;
                if duration.is_finite()
                    && duration >= 0.0
                    && compare_numbers(current + rate * duration, threshold, op)
                {
                    earliest = Some(
                        earliest
                            .map(|current| current.min(duration))
                            .unwrap_or(duration),
                    );
                }
            }
        }
    }
    Ok(earliest)
}

fn active_rate_for_feature(
    subject: &CoreSubjectRunState<'_>,
    state_id: &str,
    feature: &str,
    values: &BTreeMap<(String, String), Value>,
) -> Result<Option<f64>, CoreSimulationError> {
    let Some(state) = subject
        .machine
        .states
        .iter()
        .find(|state| state.id == state_id)
    else {
        return Ok(None);
    };
    let Some(StateDoBehavior::RateIntegration { rates }) = &state.do_behavior else {
        return Ok(None);
    };
    rates
        .iter()
        .find(|rate| rate.feature == feature)
        .map(|rate| rate_value(&rate.source, &subject.subject_id, values))
        .transpose()
}

fn comparison_against_threshold(
    expression: &str,
    subject_id: &str,
    values: &BTreeMap<(String, String), Value>,
) -> Option<(String, &'static str, f64)> {
    for op in [">=", "<=", "==", "!=", ">", "<"] {
        if let Some((left, right)) = expression.trim().split_once(op) {
            let left = left.trim();
            let right = right.trim();
            if let Some(threshold) = numeric_operand(right, subject_id, values)
                && resolve_feature_path(left, subject_id, values).is_some()
            {
                return Some((left.to_string(), op, threshold));
            }
            if let Some(threshold) = numeric_operand(left, subject_id, values)
                && resolve_feature_path(right, subject_id, values).is_some()
            {
                let reversed = match op {
                    ">=" => "<=",
                    "<=" => ">=",
                    ">" => "<",
                    "<" => ">",
                    other => other,
                };
                return Some((right.to_string(), reversed, threshold));
            }
        }
    }
    None
}

fn compare_numbers(left: f64, right: f64, op: &str) -> bool {
    match op {
        ">=" => left >= right,
        "<=" => left <= right,
        "==" => (left - right).abs() <= f64::EPSILON,
        "!=" => (left - right).abs() > f64::EPSILON,
        ">" => left > right,
        "<" => left < right,
        _ => false,
    }
}

fn rate_value(
    source: &SimulationRateSource,
    subject_id: &str,
    values: &BTreeMap<(String, String), Value>,
) -> Result<f64, CoreSimulationError> {
    match source {
        SimulationRateSource::Constant(value) => Ok(*value),
        SimulationRateSource::Feature(feature) => Ok(values
            .get(&(subject_id.to_string(), feature.clone()))
            .and_then(Value::as_f64)
            .unwrap_or_default()),
        SimulationRateSource::ExpressionIr(expression) => {
            eval_number(expression, subject_id, values)
        }
    }
}

fn guard_allows(
    guard: &Option<SimulationGuard>,
    subject_id: &str,
    values: &BTreeMap<(String, String), Value>,
) -> bool {
    match guard {
        None => true,
        Some(SimulationGuard::RuntimeFeature(feature)) => values
            .get(&(subject_id.to_string(), feature.clone()))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        Some(SimulationGuard::ExpressionIr(expression)) => {
            eval_bool(expression, subject_id, values).unwrap_or(false)
        }
    }
}

fn eval_bool(
    expression: &Value,
    subject_id: &str,
    values: &BTreeMap<(String, String), Value>,
) -> Result<bool, CoreSimulationError> {
    match eval_value(expression, subject_id, values)? {
        Value::Bool(value) => Ok(value),
        Value::Number(value) => Ok(value.as_f64().unwrap_or_default() != 0.0),
        Value::String(value) => Ok(bool_expression_string(&value, subject_id, values)),
        other => Err(CoreSimulationError::InvalidExpression(format!(
            "expected boolean, found {other}"
        ))),
    }
}

fn eval_number(
    expression: &Value,
    subject_id: &str,
    values: &BTreeMap<(String, String), Value>,
) -> Result<f64, CoreSimulationError> {
    eval_value(expression, subject_id, values)?
        .as_f64()
        .ok_or_else(|| CoreSimulationError::InvalidExpression("expected number".to_string()))
}

fn eval_value(
    expression: &Value,
    subject_id: &str,
    values: &BTreeMap<(String, String), Value>,
) -> Result<Value, CoreSimulationError> {
    let Some(object) = expression.as_object() else {
        return Ok(expression.clone());
    };
    match object.get("kind").and_then(Value::as_str) {
        Some("literal") => Ok(object.get("value").cloned().unwrap_or(Value::Null)),
        Some("path") => {
            let feature = expression_path(expression).ok_or_else(|| {
                CoreSimulationError::InvalidExpression(
                    "path expression has no segments".to_string(),
                )
            })?;
            Ok(resolve_feature_path(&feature, subject_id, values).unwrap_or(Value::Null))
        }
        Some("unary") => {
            let op = object.get("op").and_then(Value::as_str).unwrap_or_default();
            let operand = object
                .get("operand")
                .or_else(|| object.get("expr"))
                .ok_or_else(|| {
                    CoreSimulationError::InvalidExpression(
                        "unary expression has no operand".to_string(),
                    )
                })?;
            match op {
                "not" | "!" => Ok(Value::Bool(!eval_bool(operand, subject_id, values)?)),
                "-" => Ok(Value::from(-eval_number(operand, subject_id, values)?)),
                _ => Err(CoreSimulationError::InvalidExpression(format!(
                    "unsupported unary operator `{op}`"
                ))),
            }
        }
        Some("binary") => {
            let op = object.get("op").and_then(Value::as_str).unwrap_or_default();
            let left = object.get("left").ok_or_else(|| {
                CoreSimulationError::InvalidExpression(
                    "binary expression has no left operand".to_string(),
                )
            })?;
            let right = object.get("right").ok_or_else(|| {
                CoreSimulationError::InvalidExpression(
                    "binary expression has no right operand".to_string(),
                )
            })?;
            eval_binary(op, left, right, subject_id, values)
        }
        Some(other) => Err(CoreSimulationError::InvalidExpression(format!(
            "unsupported expression kind `{other}`"
        ))),
        None => Ok(expression.clone()),
    }
}

fn eval_binary(
    op: &str,
    left: &Value,
    right: &Value,
    subject_id: &str,
    values: &BTreeMap<(String, String), Value>,
) -> Result<Value, CoreSimulationError> {
    match op {
        "and" | "&&" => Ok(Value::Bool(
            eval_bool(left, subject_id, values)? && eval_bool(right, subject_id, values)?,
        )),
        "or" | "||" => Ok(Value::Bool(
            eval_bool(left, subject_id, values)? || eval_bool(right, subject_id, values)?,
        )),
        "equal" | "==" => Ok(Value::Bool(
            eval_value(left, subject_id, values)? == eval_value(right, subject_id, values)?,
        )),
        "not_equal" | "!=" => Ok(Value::Bool(
            eval_value(left, subject_id, values)? != eval_value(right, subject_id, values)?,
        )),
        "greater" | ">" => Ok(Value::Bool(
            eval_number(left, subject_id, values)? > eval_number(right, subject_id, values)?,
        )),
        "greater_equal" | ">=" => Ok(Value::Bool(
            eval_number(left, subject_id, values)? >= eval_number(right, subject_id, values)?,
        )),
        "less" | "<" => Ok(Value::Bool(
            eval_number(left, subject_id, values)? < eval_number(right, subject_id, values)?,
        )),
        "less_equal" | "<=" => Ok(Value::Bool(
            eval_number(left, subject_id, values)? <= eval_number(right, subject_id, values)?,
        )),
        "add" | "plus" | "+" => Ok(Value::from(
            eval_number(left, subject_id, values)? + eval_number(right, subject_id, values)?,
        )),
        "sub" | "subtract" | "minus" | "-" => Ok(Value::from(
            eval_number(left, subject_id, values)? - eval_number(right, subject_id, values)?,
        )),
        "mul" | "multiply" | "*" => Ok(Value::from(
            eval_number(left, subject_id, values)? * eval_number(right, subject_id, values)?,
        )),
        "div" | "divide" | "/" => Ok(Value::from(
            eval_number(left, subject_id, values)? / eval_number(right, subject_id, values)?,
        )),
        _ => Err(CoreSimulationError::InvalidExpression(format!(
            "unsupported binary operator `{op}`"
        ))),
    }
}

fn expression_path(expression: &Value) -> Option<String> {
    expression
        .get("segments")?
        .as_array()?
        .iter()
        .filter_map(|segment| {
            segment.as_str().map(ToOwned::to_owned).or_else(|| {
                segment
                    .get("name")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
        })
        .collect::<Vec<_>>()
        .join(".")
        .into()
}

fn resolve_feature_path(
    path: &str,
    subject_id: &str,
    values: &BTreeMap<(String, String), Value>,
) -> Option<Value> {
    if let Some(value) = values.get(&(subject_id.to_string(), path.to_string())) {
        return Some(value.clone());
    }
    values
        .iter()
        .filter_map(|((candidate_subject, candidate_feature), value)| {
            let prefix = format!("{candidate_subject}.");
            path.strip_prefix(&prefix)
                .filter(|feature| *feature == candidate_feature)
                .map(|_| (candidate_subject.len(), value.clone()))
        })
        .max_by_key(|(subject_len, _)| *subject_len)
        .map(|(_, value)| value)
}

fn bool_expression_string(
    expression: &str,
    subject_id: &str,
    values: &BTreeMap<(String, String), Value>,
) -> bool {
    let trimmed = expression.trim();
    if trimmed.eq_ignore_ascii_case("true") {
        return true;
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return false;
    }
    for op in [">=", "<=", "==", "!=", ">", "<"] {
        if let Some((left, right)) = trimmed.split_once(op) {
            let left = numeric_operand(left.trim(), subject_id, values);
            let right = numeric_operand(right.trim(), subject_id, values);
            if let (Some(left), Some(right)) = (left, right) {
                return match op {
                    ">=" => left >= right,
                    "<=" => left <= right,
                    "==" => (left - right).abs() <= f64::EPSILON,
                    "!=" => (left - right).abs() > f64::EPSILON,
                    ">" => left > right,
                    "<" => left < right,
                    _ => false,
                };
            }
        }
    }
    values
        .get(&(subject_id.to_string(), trimmed.to_string()))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn numeric_operand(
    operand: &str,
    subject_id: &str,
    values: &BTreeMap<(String, String), Value>,
) -> Option<f64> {
    operand.parse::<f64>().ok().or_else(|| {
        resolve_feature_path(operand, subject_id, values).and_then(|value| value.as_f64())
    })
}

fn parse_duration_s(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    let numeric = trimmed
        .strip_suffix("ms")
        .and_then(|value| value.trim().parse::<f64>().ok().map(|value| value / 1000.0))
        .or_else(|| {
            trimmed
                .strip_suffix('s')
                .and_then(|value| value.trim().parse::<f64>().ok())
        })
        .or_else(|| trimmed.parse::<f64>().ok())?;
    numeric
        .is_finite()
        .then_some(numeric)
        .filter(|value| *value >= 0.0)
}

fn apply_state_behavior(
    machine: &SimulationStateMachine,
    state_id: &str,
    subject_id: &str,
    values: &mut BTreeMap<(String, String), Value>,
    pending_signals: &mut VecDeque<CorePendingSignal>,
) {
    let Some(state) = machine.states.iter().find(|state| state.id == state_id) else {
        return;
    };
    if let Some(behavior) = &state.entry_behavior {
        apply_action_sequence(behavior, subject_id, values, pending_signals);
    }
}

fn apply_exit_behavior(
    machine: &SimulationStateMachine,
    state_id: &str,
    subject_id: &str,
    values: &mut BTreeMap<(String, String), Value>,
    pending_signals: &mut VecDeque<CorePendingSignal>,
) {
    let Some(state) = machine.states.iter().find(|state| state.id == state_id) else {
        return;
    };
    if let Some(behavior) = &state.exit_behavior {
        apply_action_sequence(behavior, subject_id, values, pending_signals);
    }
}

fn apply_action_sequence(
    sequence: &SimulationActionSequence,
    subject_id: &str,
    values: &mut BTreeMap<(String, String), Value>,
    pending_signals: &mut VecDeque<CorePendingSignal>,
) {
    for action in &sequence.actions {
        match action {
            SimulationActionNode::Effect(effect) => {
                apply_effect(effect, subject_id, values, pending_signals);
            }
            SimulationActionNode::Decision {
                guard,
                then_branch,
                else_branch,
            } => {
                let guard = Some(guard.clone());
                if guard_allows(&guard, subject_id, values) {
                    apply_action_sequence(then_branch, subject_id, values, pending_signals);
                } else if let Some(else_branch) = else_branch {
                    apply_action_sequence(else_branch, subject_id, values, pending_signals);
                }
            }
        }
    }
}

fn apply_effects(
    effects: &[SimulationEffect],
    subject_id: &str,
    values: &mut BTreeMap<(String, String), Value>,
    pending_signals: &mut VecDeque<CorePendingSignal>,
) {
    for effect in effects {
        apply_effect(effect, subject_id, values, pending_signals);
    }
}

fn apply_effect(
    effect: &SimulationEffect,
    subject_id: &str,
    values: &mut BTreeMap<(String, String), Value>,
    pending_signals: &mut VecDeque<CorePendingSignal>,
) {
    match effect {
        SimulationEffect::Assign(effect) => {
            values.insert(
                (subject_id.to_string(), effect.feature.clone()),
                effect.value.clone(),
            );
        }
        SimulationEffect::EmitSignal(effect) => {
            pending_signals.push_back(CorePendingSignal {
                source_subject_id: subject_id.to_string(),
                signal_type: effect.signal_type.clone(),
                target: effect.target.clone(),
            });
        }
        SimulationEffect::Log(_) => {}
    }
}

fn initial_configuration(
    machine: &SimulationStateMachine,
    initial_state_id: Option<&str>,
) -> Option<Vec<String>> {
    if let Some(state_id) = initial_state_id {
        return enter_state_configuration(machine, state_id);
    }
    let root = machine
        .states
        .iter()
        .find(|state| state.parent_state_id.is_none() && state.is_initial)
        .or_else(|| {
            let mut roots = machine
                .states
                .iter()
                .filter(|state| state.parent_state_id.is_none());
            let root = roots.next()?;
            roots.next().is_none().then_some(root)
        })?;
    enter_state_configuration(machine, &root.id)
}

fn enter_state_configuration(
    machine: &SimulationStateMachine,
    state_id: &str,
) -> Option<Vec<String>> {
    let mut configuration = ancestor_path(machine, state_id)?;
    append_default_descendants(machine, state_id, &mut configuration);
    Some(configuration)
}

fn append_default_descendants(
    machine: &SimulationStateMachine,
    state_id: &str,
    configuration: &mut Vec<String>,
) {
    let Some(state) = machine.states.iter().find(|state| state.id == state_id) else {
        return;
    };
    if state.is_orthogonal {
        for child in default_orthogonal_children(machine, state_id) {
            if !configuration.contains(&child.id) {
                configuration.push(child.id.clone());
            }
            append_default_descendants(machine, &child.id, configuration);
        }
        return;
    }
    if let Some(child) = default_child_state(machine, state_id) {
        if !configuration.contains(&child.id) {
            configuration.push(child.id.clone());
        }
        append_default_descendants(machine, &child.id, configuration);
    }
}

fn ancestor_path(machine: &SimulationStateMachine, state_id: &str) -> Option<Vec<String>> {
    let mut path = Vec::new();
    let mut cursor = machine.states.iter().find(|state| state.id == state_id)?;
    loop {
        path.push(cursor.id.clone());
        let Some(parent_id) = &cursor.parent_state_id else {
            path.reverse();
            return Some(path);
        };
        cursor = machine.states.iter().find(|state| state.id == *parent_id)?;
    }
}

fn default_child_state<'a>(
    machine: &'a SimulationStateMachine,
    parent_id: &str,
) -> Option<&'a SimulationState> {
    machine
        .states
        .iter()
        .filter(|state| state.parent_state_id.as_deref() == Some(parent_id) && !state.is_history)
        .find(|state| state.is_initial)
        .or_else(|| {
            machine.states.iter().find(|state| {
                state.parent_state_id.as_deref() == Some(parent_id) && !state.is_history
            })
        })
}

fn default_orthogonal_children<'a>(
    machine: &'a SimulationStateMachine,
    parent_id: &str,
) -> Vec<&'a SimulationState> {
    let initial = machine
        .states
        .iter()
        .filter(|state| {
            state.parent_state_id.as_deref() == Some(parent_id)
                && state.is_initial
                && !state.is_history
        })
        .collect::<Vec<_>>();
    if !initial.is_empty() {
        return initial;
    }
    machine
        .states
        .iter()
        .filter(|state| state.parent_state_id.as_deref() == Some(parent_id) && !state.is_history)
        .collect()
}

fn is_descendant_of(machine: &SimulationStateMachine, state_id: &str, ancestor_id: &str) -> bool {
    let mut cursor = machine.states.iter().find(|state| state.id == state_id);
    while let Some(state) = cursor {
        let Some(parent_id) = &state.parent_state_id else {
            return false;
        };
        if parent_id == ancestor_id {
            return true;
        }
        cursor = machine
            .states
            .iter()
            .find(|candidate| candidate.id == *parent_id);
    }
    false
}

fn resolve_history_target(
    machine: &SimulationStateMachine,
    subject_id: &str,
    target_state_id: &str,
    history: &BTreeMap<(String, String), String>,
) -> Option<String> {
    let target = machine
        .states
        .iter()
        .find(|state| state.id == target_state_id)?;
    if !target.is_history {
        return Some(target_state_id.to_string());
    }
    let parent_id = target.parent_state_id.as_ref()?;
    history
        .get(&(subject_id.to_string(), parent_id.clone()))
        .cloned()
        .or_else(|| default_child_state(machine, parent_id).map(|state| state.id.clone()))
}

fn record_shallow_history(
    machine: &SimulationStateMachine,
    subject_id: &str,
    state_id: &str,
    history: &mut BTreeMap<(String, String), String>,
) {
    let Some(state) = machine.states.iter().find(|state| state.id == state_id) else {
        return;
    };
    let Some(parent_id) = &state.parent_state_id else {
        return;
    };
    history.insert(
        (subject_id.to_string(), parent_id.clone()),
        state_id.to_string(),
    );
}

pub fn validate_simulation_model(model: &SimulationModel) -> Result<(), SimulationProfileError> {
    let mut findings = Vec::new();
    if model.machines.is_empty() {
        findings.push(finding(
            "model.no_machines",
            "Simulation model has no state machines.",
            None,
            None,
        ));
    }

    for machine in &model.machines {
        validate_machine(machine, &mut findings);
    }
    if let Err(error) = derived_rule_order(&model.derived_rules) {
        findings.extend(error.findings);
    }
    validate_binding_rules(&model.binding_rules, &mut findings);

    if findings.is_empty() {
        Ok(())
    } else {
        Err(SimulationProfileError { findings })
    }
}

fn validate_binding_rules(
    rules: &[SimulationBindingRule],
    findings: &mut Vec<SimulationProfileFinding>,
) {
    for rule in rules {
        if rule.left.feature.trim().is_empty() || rule.right.feature.trim().is_empty() {
            findings.push(finding(
                "binding_rule.feature_missing",
                "Binding rule endpoints must name features.",
                None,
                Some(&rule.id),
            ));
        }
        if rule.left.subject_id.is_none() && rule.right.subject_id.is_none() {
            findings.push(finding(
                "binding_rule.ambiguous_endpoint",
                "Binding rules must scope at least one endpoint to a subject.",
                None,
                Some(&rule.id),
            ));
        }
    }
}

fn validate_machine(
    machine: &SimulationStateMachine,
    findings: &mut Vec<SimulationProfileFinding>,
) {
    let mut state_ids = BTreeSet::new();
    for state in &machine.states {
        if !state_ids.insert(state.id.clone()) {
            findings.push(finding(
                "state.duplicate_id",
                "State IDs must be unique within a simulation machine.",
                Some(&machine.id),
                Some(&state.id),
            ));
        }
    }

    if machine.states.is_empty() {
        findings.push(finding(
            "machine.no_states",
            "State machine has no states.",
            Some(&machine.id),
            None,
        ));
    }

    let top_level_count = machine
        .states
        .iter()
        .filter(|state| state.parent_state_id.is_none())
        .count();
    let top_initial_count = machine
        .states
        .iter()
        .filter(|state| state.parent_state_id.is_none() && state.is_initial)
        .count();
    if top_initial_count == 0 && top_level_count > 1 {
        findings.push(finding(
            "machine.no_initial_state",
            "State machine with multiple top-level states must have a top-level initial state.",
            Some(&machine.id),
            None,
        ));
    }
    if top_initial_count > 1 {
        findings.push(finding(
            "machine.multiple_initial_states",
            "State machine has more than one top-level initial state.",
            Some(&machine.id),
            None,
        ));
    }

    for state in &machine.states {
        if let Some(parent_id) = &state.parent_state_id
            && !state_ids.contains(parent_id)
        {
            findings.push(finding(
                "state.missing_parent",
                "State parent must reference another state in the same machine.",
                Some(&machine.id),
                Some(&state.id),
            ));
        }
    }

    for parent in &machine.states {
        let initial_child_count = machine
            .states
            .iter()
            .filter(|state| {
                state.parent_state_id.as_deref() == Some(parent.id.as_str()) && state.is_initial
            })
            .count();
        if initial_child_count > 1 && !parent.is_orthogonal {
            findings.push(finding(
                "state.multiple_initial_children",
                "Compound state has multiple initial children but is not marked orthogonal.",
                Some(&machine.id),
                Some(&parent.id),
            ));
        }
    }

    let mut transition_keys =
        BTreeMap::<(String, SimulationTriggerKind, Option<String>), usize>::new();
    for transition in &machine.transitions {
        if !state_ids.contains(&transition.source) {
            findings.push(finding(
                "transition.missing_source",
                "Transition source must reference a state in the same machine.",
                Some(&machine.id),
                Some(&transition.id),
            ));
        }
        if !state_ids.contains(&transition.target) {
            findings.push(finding(
                "transition.missing_target",
                "Transition target must reference a state in the same machine.",
                Some(&machine.id),
                Some(&transition.id),
            ));
        }
        if matches!(
            transition.trigger.kind,
            SimulationTriggerKind::Event
                | SimulationTriggerKind::Signal
                | SimulationTriggerKind::After
                | SimulationTriggerKind::Time
                | SimulationTriggerKind::Change
        ) && transition
            .trigger
            .value
            .as_deref()
            .unwrap_or_default()
            .is_empty()
        {
            findings.push(finding(
                "transition.missing_trigger",
                "Triggered transitions must declare a trigger value.",
                Some(&machine.id),
                Some(&transition.id),
            ));
        }
        let key = (
            transition.source.clone(),
            transition.trigger.kind.clone(),
            transition.trigger.value.clone(),
        );
        *transition_keys.entry(key).or_default() += 1;
    }

    for ((source, _, trigger), count) in transition_keys {
        if count > 1 {
            findings.push(finding(
                "transition.ambiguous_trigger",
                &format!(
                    "Source state `{source}` has {count} transitions for trigger `{}`.",
                    trigger.unwrap_or_else(|| "<none>".to_string())
                ),
                Some(&machine.id),
                Some(&source),
            ));
        }
    }
}

fn finding(
    code: &str,
    message: &str,
    machine_id: Option<&str>,
    element_id: Option<&str>,
) -> SimulationProfileFinding {
    SimulationProfileFinding {
        code: code.to_string(),
        message: message.to_string(),
        machine_id: machine_id.map(str::to_string),
        element_id: element_id.map(str::to_string),
    }
}

fn default_step_duration() -> f64 {
    1.0
}

pub mod tuple_value_map {
    use std::collections::BTreeMap;

    use serde::ser::SerializeSeq;
    use serde::{Deserialize, Deserializer, Serializer};
    use serde_json::{Value, json};

    pub fn serialize<S>(
        values: &BTreeMap<(String, String), Value>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(values.len()))?;
        for ((subject, feature), value) in values {
            sequence.serialize_element(&json!({
                "subject": subject,
                "feature": feature,
                "value": value,
            }))?;
        }
        sequence.end()
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<(String, String), Value>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let mut values = BTreeMap::new();
        match value {
            Value::Array(entries) => {
                for entry in entries {
                    let object = entry.as_object().ok_or_else(|| {
                        serde::de::Error::custom("tuple value entry must be an object")
                    })?;
                    let subject =
                        object
                            .get("subject")
                            .and_then(Value::as_str)
                            .ok_or_else(|| {
                                serde::de::Error::custom("tuple value entry missing `subject`")
                            })?;
                    let feature =
                        object
                            .get("feature")
                            .and_then(Value::as_str)
                            .ok_or_else(|| {
                                serde::de::Error::custom("tuple value entry missing `feature`")
                            })?;
                    let entry_value = object.get("value").cloned().unwrap_or(Value::Null);
                    values.insert((subject.to_string(), feature.to_string()), entry_value);
                }
            }
            Value::Object(entries) => {
                for (key, entry_value) in entries {
                    let Some((subject, feature)) = key.split_once('|') else {
                        return Err(serde::de::Error::custom(format!(
                            "invalid tuple key `{key}`, expected `subject|feature`"
                        )));
                    };
                    values.insert((subject.to_string(), feature.to_string()), entry_value);
                }
            }
            _ => {
                return Err(serde::de::Error::custom(
                    "tuple value map must be an array of entries or a legacy map",
                ));
            }
        }
        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validator_rejects_ambiguous_transitions() {
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![state("s1", true), state("s2", false), state("s3", false)],
                transitions: vec![
                    transition("t1", "s1", "s2", "go"),
                    transition("t2", "s1", "s3", "go"),
                ],
            }],
            derived_rules: Vec::new(),
            binding_rules: Vec::new(),
        };

        let error = validate_simulation_model(&model).unwrap_err();
        assert!(
            error
                .findings
                .iter()
                .any(|finding| finding.code == "transition.ambiguous_trigger")
        );
    }

    #[test]
    fn validator_accepts_minimal_machine() {
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![state("s1", true), state("s2", false)],
                transitions: vec![transition("t1", "s1", "s2", "go")],
            }],
            derived_rules: Vec::new(),
            binding_rules: Vec::new(),
        };

        validate_simulation_model(&model).unwrap();
    }

    #[test]
    fn core_runner_rejects_invalid_model_before_execution() {
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![state("s1", true), state("s2", false)],
                transitions: vec![
                    transition("t1", "s1", "s2", "go"),
                    transition("t2", "s1", "s2", "go"),
                ],
            }],
            derived_rules: Vec::new(),
            binding_rules: Vec::new(),
        };

        let error = run_concurrent_simulation_model(
            &model,
            ConcurrentSimulationScenario {
                id: "scenario".to_string(),
                subjects: vec![ConcurrentSubjectScenario {
                    subject_id: "subject".to_string(),
                    machine_id: "Machine".to_string(),
                    initial_state_id: None,
                    events: vec![SimulationEvent {
                        id: "event.go".to_string(),
                        trigger: "go".to_string(),
                    }],
                }],
                max_steps: 4,
                step_duration_s: 1.0,
                clock_config: None,
                initial_values: BTreeMap::new(),
                requirements: Vec::new(),
                objectives: Vec::new(),
            },
            SimulationClockConfig::default(),
        )
        .unwrap_err();

        assert!(matches!(error, CoreSimulationError::InvalidProfile(_)));
    }

    #[test]
    fn core_runner_executes_event_signal_flow() {
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![
                SimulationStateMachine {
                    id: "BedMachine".to_string(),
                    label: "BedMachine".to_string(),
                    states: vec![state("bed.heating", true), state("bed.ready", false)],
                    transitions: vec![SimulationTransition {
                        id: "bed.ready".to_string(),
                        source: "bed.heating".to_string(),
                        target: "bed.ready".to_string(),
                        trigger: SimulationTrigger {
                            kind: SimulationTriggerKind::Event,
                            value: Some("finish".to_string()),
                        },
                        guard: None,
                        effects: vec![SimulationEffect::EmitSignal(SignalEffect {
                            signal_type: "BedReady".to_string(),
                            target: Some("printer".to_string()),
                        })],
                    }],
                },
                SimulationStateMachine {
                    id: "PrinterMachine".to_string(),
                    label: "PrinterMachine".to_string(),
                    states: vec![
                        state("printer.heating", true),
                        state("printer.printing", false),
                    ],
                    transitions: vec![SimulationTransition {
                        id: "printer.print".to_string(),
                        source: "printer.heating".to_string(),
                        target: "printer.printing".to_string(),
                        trigger: SimulationTrigger {
                            kind: SimulationTriggerKind::Signal,
                            value: Some("BedReady".to_string()),
                        },
                        guard: None,
                        effects: Vec::new(),
                    }],
                },
            ],
            derived_rules: Vec::new(),
            binding_rules: Vec::new(),
        };

        let trace = run_concurrent_simulation_model(
            &model,
            ConcurrentSimulationScenario {
                id: "scenario".to_string(),
                subjects: vec![
                    ConcurrentSubjectScenario {
                        subject_id: "bed".to_string(),
                        machine_id: "BedMachine".to_string(),
                        initial_state_id: None,
                        events: vec![SimulationEvent {
                            id: "event.finish".to_string(),
                            trigger: "finish".to_string(),
                        }],
                    },
                    ConcurrentSubjectScenario {
                        subject_id: "printer".to_string(),
                        machine_id: "PrinterMachine".to_string(),
                        initial_state_id: None,
                        events: Vec::new(),
                    },
                ],
                max_steps: 6,
                step_duration_s: 1.0,
                clock_config: None,
                initial_values: BTreeMap::new(),
                requirements: Vec::new(),
                objectives: Vec::new(),
            },
            SimulationClockConfig::default(),
        )
        .unwrap();

        assert!(trace.timeline.iter().any(|entry| {
            entry
                .states
                .get("printer")
                .is_some_and(|states| states == &vec!["printer.printing".to_string()])
        }));
    }

    #[test]
    fn core_runner_executes_completion_and_after_transitions() {
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![
                    state("idle", true),
                    state("armed", false),
                    state("done", false),
                ],
                transitions: vec![
                    SimulationTransition {
                        id: "idle.armed".to_string(),
                        source: "idle".to_string(),
                        target: "armed".to_string(),
                        trigger: SimulationTrigger {
                            kind: SimulationTriggerKind::Completion,
                            value: None,
                        },
                        guard: None,
                        effects: Vec::new(),
                    },
                    SimulationTransition {
                        id: "armed.done".to_string(),
                        source: "armed".to_string(),
                        target: "done".to_string(),
                        trigger: SimulationTrigger {
                            kind: SimulationTriggerKind::After,
                            value: Some("2s".to_string()),
                        },
                        guard: None,
                        effects: Vec::new(),
                    },
                ],
            }],
            derived_rules: Vec::new(),
            binding_rules: Vec::new(),
        };

        let trace = run_concurrent_simulation_model(
            &model,
            ConcurrentSimulationScenario {
                id: "scenario".to_string(),
                subjects: vec![ConcurrentSubjectScenario {
                    subject_id: "subject".to_string(),
                    machine_id: "Machine".to_string(),
                    initial_state_id: None,
                    events: Vec::new(),
                }],
                max_steps: 8,
                step_duration_s: 1.0,
                clock_config: None,
                initial_values: BTreeMap::new(),
                requirements: Vec::new(),
                objectives: Vec::new(),
            },
            SimulationClockConfig::default(),
        )
        .unwrap();

        let done = trace
            .timeline
            .iter()
            .find(|entry| {
                entry
                    .states
                    .get("subject")
                    .is_some_and(|states| states == &vec!["done".to_string()])
            })
            .unwrap();
        assert_eq!(done.t, 2.0);
    }

    #[test]
    fn core_runner_integrates_state_rate_to_change_guard_crossing() {
        let heating = SimulationState {
            do_behavior: Some(StateDoBehavior::RateIntegration {
                rates: vec![SimulationRate {
                    feature: "temperature".to_string(),
                    source: SimulationRateSource::Feature("heatRate".to_string()),
                }],
            }),
            ..state("heating", true)
        };
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![heating, state("ready", false)],
                transitions: vec![SimulationTransition {
                    id: "heating.ready".to_string(),
                    source: "heating".to_string(),
                    target: "ready".to_string(),
                    trigger: SimulationTrigger {
                        kind: SimulationTriggerKind::Change,
                        value: Some("temperature >= target".to_string()),
                    },
                    guard: None,
                    effects: Vec::new(),
                }],
            }],
            derived_rules: Vec::new(),
            binding_rules: Vec::new(),
        };

        let trace = run_concurrent_simulation_model(
            &model,
            ConcurrentSimulationScenario {
                id: "scenario".to_string(),
                subjects: vec![ConcurrentSubjectScenario {
                    subject_id: "bed".to_string(),
                    machine_id: "Machine".to_string(),
                    initial_state_id: None,
                    events: Vec::new(),
                }],
                max_steps: 100,
                step_duration_s: 1.0,
                clock_config: None,
                initial_values: BTreeMap::from([
                    (
                        ("bed".to_string(), "temperature".to_string()),
                        Value::from(22.0),
                    ),
                    (
                        ("bed".to_string(), "heatRate".to_string()),
                        Value::from(2.3),
                    ),
                    (
                        ("bed".to_string(), "target".to_string()),
                        Value::from(110.0),
                    ),
                ]),
                requirements: Vec::new(),
                objectives: Vec::new(),
            },
            SimulationClockConfig::default(),
        )
        .unwrap();

        let ready = trace
            .timeline
            .iter()
            .find(|entry| {
                entry
                    .states
                    .get("bed")
                    .is_some_and(|states| states == &vec!["ready".to_string()])
            })
            .unwrap();
        assert!((ready.t - ((110.0 - 22.0) / 2.3)).abs() <= 0.1);
    }

    #[test]
    fn trace_values_serialize_as_typed_entries_and_read_legacy_maps() {
        let entry = SimTraceEntry {
            t: 0.0,
            states: BTreeMap::new(),
            values: BTreeMap::from([(
                ("bed".to_string(), "temperature".to_string()),
                Value::from(22.0),
            )]),
            events: Vec::new(),
        };

        let encoded = serde_json::to_value(&entry).unwrap();
        let values = encoded.get("values").and_then(Value::as_array).unwrap();
        assert_eq!(
            values[0].get("subject").and_then(Value::as_str),
            Some("bed")
        );
        assert_eq!(
            values[0].get("feature").and_then(Value::as_str),
            Some("temperature")
        );
        assert_eq!(values[0].get("value").and_then(Value::as_f64), Some(22.0));

        let decoded: SimTraceEntry = serde_json::from_value(encoded).unwrap();
        assert_eq!(
            decoded
                .values
                .get(&("bed".to_string(), "temperature".to_string())),
            Some(&Value::from(22.0))
        );

        let legacy: SimTraceEntry = serde_json::from_value(serde_json::json!({
            "t": 0.0,
            "states": {},
            "values": { "bed|temperature": 23.0 },
            "events": []
        }))
        .unwrap();
        assert_eq!(
            legacy
                .values
                .get(&("bed".to_string(), "temperature".to_string())),
            Some(&Value::from(23.0))
        );
    }

    #[test]
    fn core_runner_tags_rate_integrated_channels() {
        let heating = SimulationState {
            do_behavior: Some(StateDoBehavior::RateIntegration {
                rates: vec![SimulationRate {
                    feature: "temperature".to_string(),
                    source: SimulationRateSource::Constant(2.0),
                }],
            }),
            ..state("heating", true)
        };
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![heating],
                transitions: Vec::new(),
            }],
            derived_rules: Vec::new(),
            binding_rules: Vec::new(),
        };

        let trace = run_concurrent_simulation_model(
            &model,
            ConcurrentSimulationScenario {
                id: "scenario".to_string(),
                subjects: vec![ConcurrentSubjectScenario {
                    subject_id: "bed".to_string(),
                    machine_id: "Machine".to_string(),
                    initial_state_id: None,
                    events: Vec::new(),
                }],
                max_steps: 1,
                step_duration_s: 1.0,
                clock_config: None,
                initial_values: BTreeMap::from([(
                    ("bed".to_string(), "temperature".to_string()),
                    Value::from(22.0),
                )]),
                requirements: Vec::new(),
                objectives: Vec::new(),
            },
            SimulationClockConfig::default(),
        )
        .unwrap();

        assert!(trace.channels.iter().any(|channel| {
            channel.id == "bed.temperature" && channel.source == SimTraceChannelSource::RateEffect
        }));
    }

    #[test]
    fn core_runner_applies_lookup_table_do_behavior() {
        let heating = SimulationState {
            do_behavior: Some(StateDoBehavior::LookupTable {
                tables: vec![SimulationLookupTable {
                    feature: "temperature".to_string(),
                    samples: vec![
                        SimulationLookupSample {
                            t: 0.0,
                            value: 20.0,
                        },
                        SimulationLookupSample {
                            t: 5.0,
                            value: 60.0,
                        },
                        SimulationLookupSample {
                            t: 10.0,
                            value: 100.0,
                        },
                    ],
                }],
            }),
            ..state("heating", true)
        };
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![heating],
                transitions: Vec::new(),
            }],
            derived_rules: Vec::new(),
            binding_rules: Vec::new(),
        };

        let trace = run_concurrent_simulation_model(
            &model,
            ConcurrentSimulationScenario {
                id: "scenario".to_string(),
                subjects: vec![ConcurrentSubjectScenario {
                    subject_id: "bed".to_string(),
                    machine_id: "Machine".to_string(),
                    initial_state_id: None,
                    events: Vec::new(),
                }],
                max_steps: 2,
                step_duration_s: 2.5,
                clock_config: Some(SimulationClockConfig {
                    max_time_s: 5.0,
                    fixed_step_s: 2.5,
                    sample_interval_s: 2.5,
                    change_loop_limit: 20,
                }),
                initial_values: BTreeMap::new(),
                requirements: Vec::new(),
                objectives: Vec::new(),
            },
            SimulationClockConfig {
                max_time_s: 5.0,
                fixed_step_s: 2.5,
                sample_interval_s: 2.5,
                change_loop_limit: 20,
            },
        )
        .unwrap();

        assert_eq!(
            trace
                .timeline
                .first()
                .unwrap()
                .values
                .get(&("bed".to_string(), "temperature".to_string())),
            Some(&Value::from(20.0))
        );
        let mid_temperature = trace
            .timeline
            .iter()
            .find(|entry| (entry.t - 2.5).abs() <= f64::EPSILON)
            .unwrap()
            .values
            .get(&("bed".to_string(), "temperature".to_string()))
            .and_then(Value::as_f64)
            .unwrap();
        assert!((mid_temperature - 40.0).abs() <= f64::EPSILON);
        let final_temperature = trace
            .timeline
            .last()
            .unwrap()
            .values
            .get(&("bed".to_string(), "temperature".to_string()))
            .and_then(Value::as_f64)
            .unwrap();
        assert!((final_temperature - 60.0).abs() <= f64::EPSILON);
        assert!(trace.channels.iter().any(|channel| {
            channel.id == "bed.temperature" && channel.source == SimTraceChannelSource::LookupTable
        }));
    }

    #[test]
    fn expression_rates_use_rk4_for_stiff_single_step_decay() {
        let decaying = SimulationState {
            do_behavior: Some(StateDoBehavior::RateIntegration {
                rates: vec![SimulationRate {
                    feature: "x".to_string(),
                    source: SimulationRateSource::ExpressionIr(serde_json::json!({
                        "kind": "binary",
                        "op": "multiply",
                        "left": { "kind": "literal", "value": -1.0 },
                        "right": { "kind": "path", "segments": ["x"] }
                    })),
                }],
            }),
            ..state("decaying", true)
        };
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![decaying],
                transitions: Vec::new(),
            }],
            derived_rules: Vec::new(),
            binding_rules: Vec::new(),
        };

        let trace = run_concurrent_simulation_model(
            &model,
            ConcurrentSimulationScenario {
                id: "scenario".to_string(),
                subjects: vec![ConcurrentSubjectScenario {
                    subject_id: "subject".to_string(),
                    machine_id: "Machine".to_string(),
                    initial_state_id: None,
                    events: Vec::new(),
                }],
                max_steps: 1,
                step_duration_s: 1.0,
                clock_config: None,
                initial_values: BTreeMap::from([(
                    ("subject".to_string(), "x".to_string()),
                    Value::from(1.0),
                )]),
                requirements: Vec::new(),
                objectives: Vec::new(),
            },
            SimulationClockConfig {
                max_time_s: 1.0,
                fixed_step_s: 1.0,
                sample_interval_s: 1.0,
                change_loop_limit: 20,
            },
        )
        .unwrap();

        let final_x = trace
            .timeline
            .last()
            .unwrap()
            .values
            .get(&("subject".to_string(), "x".to_string()))
            .and_then(Value::as_f64)
            .unwrap();
        let expected = f64::exp(-1.0);
        assert!(
            (final_x - expected).abs() < 0.01,
            "final_x={final_x}, expected={expected}"
        );
    }

    #[test]
    fn derived_rules_propagate_after_rate_integration() {
        let heating = SimulationState {
            do_behavior: Some(StateDoBehavior::RateIntegration {
                rates: vec![SimulationRate {
                    feature: "temperature".to_string(),
                    source: SimulationRateSource::Constant(2.0),
                }],
            }),
            ..state("heating", true)
        };
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![heating],
                transitions: Vec::new(),
            }],
            derived_rules: vec![SimulationDerivedFeatureRule {
                id: "rule.margin".to_string(),
                label: "margin".to_string(),
                subject_id: Some("bed".to_string()),
                feature: "margin".to_string(),
                expression: serde_json::json!({
                    "kind": "binary",
                    "op": "subtract",
                    "left": { "kind": "path", "segments": ["target"] },
                    "right": { "kind": "path", "segments": ["temperature"] }
                }),
            }],
            binding_rules: Vec::new(),
        };

        let trace = run_concurrent_simulation_model(
            &model,
            ConcurrentSimulationScenario {
                id: "scenario".to_string(),
                subjects: vec![ConcurrentSubjectScenario {
                    subject_id: "bed".to_string(),
                    machine_id: "Machine".to_string(),
                    initial_state_id: None,
                    events: Vec::new(),
                }],
                max_steps: 1,
                step_duration_s: 1.0,
                clock_config: None,
                initial_values: BTreeMap::from([
                    (
                        ("bed".to_string(), "temperature".to_string()),
                        Value::from(20.0),
                    ),
                    (("bed".to_string(), "target".to_string()), Value::from(30.0)),
                ]),
                requirements: Vec::new(),
                objectives: Vec::new(),
            },
            SimulationClockConfig {
                max_time_s: 1.0,
                fixed_step_s: 1.0,
                sample_interval_s: 1.0,
                change_loop_limit: 20,
            },
        )
        .unwrap();

        let final_margin = trace
            .timeline
            .last()
            .unwrap()
            .values
            .get(&("bed".to_string(), "margin".to_string()))
            .and_then(Value::as_f64)
            .unwrap();
        assert!((final_margin - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn validator_rejects_derived_rule_cycles() {
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![state("idle", true)],
                transitions: Vec::new(),
            }],
            derived_rules: vec![
                SimulationDerivedFeatureRule {
                    id: "rule.a".to_string(),
                    label: "a".to_string(),
                    subject_id: None,
                    feature: "a".to_string(),
                    expression: serde_json::json!({
                        "kind": "path",
                        "segments": ["b"]
                    }),
                },
                SimulationDerivedFeatureRule {
                    id: "rule.b".to_string(),
                    label: "b".to_string(),
                    subject_id: None,
                    feature: "b".to_string(),
                    expression: serde_json::json!({
                        "kind": "path",
                        "segments": ["a"]
                    }),
                },
            ],
            binding_rules: Vec::new(),
        };

        let error = validate_simulation_model(&model).unwrap_err();
        assert!(
            error
                .findings
                .iter()
                .any(|finding| finding.code == "derived_rule.cycle")
        );
    }

    #[test]
    fn binding_rules_copy_known_values_across_subjects() {
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![state("idle", true)],
                transitions: Vec::new(),
            }],
            derived_rules: Vec::new(),
            binding_rules: vec![SimulationBindingRule {
                id: "binding.temperature".to_string(),
                label: "temperature binding".to_string(),
                left: SimulationFeatureRef {
                    subject_id: Some("bed".to_string()),
                    feature: "temperature".to_string(),
                },
                right: SimulationFeatureRef {
                    subject_id: Some("printer".to_string()),
                    feature: "bed_temperature".to_string(),
                },
            }],
        };

        let trace = run_concurrent_simulation_model(
            &model,
            ConcurrentSimulationScenario {
                id: "scenario".to_string(),
                subjects: vec![
                    ConcurrentSubjectScenario {
                        subject_id: "bed".to_string(),
                        machine_id: "Machine".to_string(),
                        initial_state_id: None,
                        events: Vec::new(),
                    },
                    ConcurrentSubjectScenario {
                        subject_id: "printer".to_string(),
                        machine_id: "Machine".to_string(),
                        initial_state_id: None,
                        events: Vec::new(),
                    },
                ],
                max_steps: 1,
                step_duration_s: 1.0,
                clock_config: None,
                initial_values: BTreeMap::from([(
                    ("bed".to_string(), "temperature".to_string()),
                    Value::from(42.0),
                )]),
                requirements: Vec::new(),
                objectives: Vec::new(),
            },
            SimulationClockConfig::default(),
        )
        .unwrap();

        assert_eq!(
            trace
                .timeline
                .first()
                .unwrap()
                .values
                .get(&("printer".to_string(), "bed_temperature".to_string()))
                .and_then(Value::as_f64),
            Some(42.0)
        );
    }

    #[test]
    fn entry_decision_nodes_select_branch_from_runtime_values() {
        let idle = SimulationState {
            entry_behavior: Some(SimulationActionSequence {
                actions: vec![SimulationActionNode::Decision {
                    guard: SimulationGuard::RuntimeFeature("hot".to_string()),
                    then_branch: SimulationActionSequence {
                        actions: vec![SimulationActionNode::Effect(SimulationEffect::Assign(
                            AssignEffect {
                                feature: "status".to_string(),
                                value: Value::from("ready"),
                            },
                        ))],
                    },
                    else_branch: Some(SimulationActionSequence {
                        actions: vec![SimulationActionNode::Effect(SimulationEffect::Assign(
                            AssignEffect {
                                feature: "status".to_string(),
                                value: Value::from("cold"),
                            },
                        ))],
                    }),
                }],
            }),
            ..state("idle", true)
        };
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![idle],
                transitions: Vec::new(),
            }],
            derived_rules: Vec::new(),
            binding_rules: Vec::new(),
        };

        let trace = run_concurrent_simulation_model(
            &model,
            ConcurrentSimulationScenario {
                id: "scenario".to_string(),
                subjects: vec![ConcurrentSubjectScenario {
                    subject_id: "bed".to_string(),
                    machine_id: "Machine".to_string(),
                    initial_state_id: None,
                    events: Vec::new(),
                }],
                max_steps: 1,
                step_duration_s: 1.0,
                clock_config: None,
                initial_values: BTreeMap::from([(
                    ("bed".to_string(), "hot".to_string()),
                    Value::from(true),
                )]),
                requirements: Vec::new(),
                objectives: Vec::new(),
            },
            SimulationClockConfig::default(),
        )
        .unwrap();

        assert_eq!(
            trace
                .timeline
                .first()
                .unwrap()
                .values
                .get(&("bed".to_string(), "status".to_string()))
                .and_then(Value::as_str),
            Some("ready")
        );
    }

    #[test]
    fn core_runner_marks_unmatched_events_as_blocked_diagnostics() {
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![state("idle", true), state("done", false)],
                transitions: vec![transition("idle.done", "idle", "done", "go")],
            }],
            derived_rules: Vec::new(),
            binding_rules: Vec::new(),
        };

        let trace = run_concurrent_simulation_model(
            &model,
            ConcurrentSimulationScenario {
                id: "scenario".to_string(),
                subjects: vec![ConcurrentSubjectScenario {
                    subject_id: "subject".to_string(),
                    machine_id: "Machine".to_string(),
                    initial_state_id: None,
                    events: vec![SimulationEvent {
                        id: "event.stop".to_string(),
                        trigger: "stop".to_string(),
                    }],
                }],
                max_steps: 4,
                step_duration_s: 1.0,
                clock_config: None,
                initial_values: BTreeMap::new(),
                requirements: Vec::new(),
                objectives: Vec::new(),
            },
            SimulationClockConfig::default(),
        )
        .unwrap();

        assert_eq!(trace.status, SimulationStatus::Blocked);
        assert!(trace.timeline.iter().any(|entry| {
            entry.events.iter().any(|event| {
                event.kind == "event.dropped"
                    && event.subject_id.as_deref() == Some("subject")
                    && event.trigger.as_deref() == Some("stop")
                    && event.reason.as_deref()
                        == Some("no enabled transition matched event trigger")
            })
        }));
    }

    #[test]
    fn core_runner_honors_explicit_clock_max_time() {
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![state("waiting", true), state("done", false)],
                transitions: vec![SimulationTransition {
                    id: "waiting.done".to_string(),
                    source: "waiting".to_string(),
                    target: "done".to_string(),
                    trigger: SimulationTrigger {
                        kind: SimulationTriggerKind::After,
                        value: Some("10s".to_string()),
                    },
                    guard: None,
                    effects: Vec::new(),
                }],
            }],
            derived_rules: Vec::new(),
            binding_rules: Vec::new(),
        };

        let trace = run_concurrent_simulation_model(
            &model,
            ConcurrentSimulationScenario {
                id: "scenario".to_string(),
                subjects: vec![ConcurrentSubjectScenario {
                    subject_id: "subject".to_string(),
                    machine_id: "Machine".to_string(),
                    initial_state_id: None,
                    events: Vec::new(),
                }],
                max_steps: 100,
                step_duration_s: 1.0,
                clock_config: Some(SimulationClockConfig {
                    max_time_s: 2.0,
                    fixed_step_s: 1.0,
                    sample_interval_s: 1.0,
                    change_loop_limit: 20,
                }),
                initial_values: BTreeMap::new(),
                requirements: Vec::new(),
                objectives: Vec::new(),
            },
            SimulationClockConfig {
                max_time_s: 2.0,
                fixed_step_s: 1.0,
                sample_interval_s: 1.0,
                change_loop_limit: 20,
            },
        )
        .unwrap();

        assert!(trace.timeline.last().is_some_and(|entry| entry.t <= 2.0));
        assert!(!trace.timeline.iter().any(|entry| {
            entry
                .states
                .get("subject")
                .is_some_and(|states| states == &vec!["done".to_string()])
        }));
    }

    #[test]
    fn core_runner_reports_change_loop_limit_when_immediate_transition_remains() {
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![SimulationStateMachine {
                id: "Machine".to_string(),
                label: "Machine".to_string(),
                states: vec![state("a", true), state("b", false)],
                transitions: vec![
                    SimulationTransition {
                        id: "a.b".to_string(),
                        source: "a".to_string(),
                        target: "b".to_string(),
                        trigger: SimulationTrigger {
                            kind: SimulationTriggerKind::Completion,
                            value: None,
                        },
                        guard: None,
                        effects: Vec::new(),
                    },
                    SimulationTransition {
                        id: "b.a".to_string(),
                        source: "b".to_string(),
                        target: "a".to_string(),
                        trigger: SimulationTrigger {
                            kind: SimulationTriggerKind::Completion,
                            value: None,
                        },
                        guard: None,
                        effects: Vec::new(),
                    },
                ],
            }],
            derived_rules: Vec::new(),
            binding_rules: Vec::new(),
        };

        let trace = run_concurrent_simulation_model(
            &model,
            ConcurrentSimulationScenario {
                id: "scenario".to_string(),
                subjects: vec![ConcurrentSubjectScenario {
                    subject_id: "subject".to_string(),
                    machine_id: "Machine".to_string(),
                    initial_state_id: None,
                    events: Vec::new(),
                }],
                max_steps: 10,
                step_duration_s: 1.0,
                clock_config: None,
                initial_values: BTreeMap::new(),
                requirements: Vec::new(),
                objectives: Vec::new(),
            },
            SimulationClockConfig {
                max_time_s: 10.0,
                fixed_step_s: 1.0,
                sample_interval_s: 1.0,
                change_loop_limit: 1,
            },
        )
        .unwrap();

        assert!(trace.timeline.iter().any(|entry| {
            entry.events.iter().any(|event| {
                event.kind == "change.loop.limit"
                    && event
                        .reason
                        .as_deref()
                        .is_some_and(|reason| reason.contains("configured limit 1"))
            })
        }));
    }

    #[test]
    fn core_runner_delivers_completion_emitted_signals_without_scripted_events() {
        let model = SimulationModel {
            id: "demo".to_string(),
            machines: vec![
                SimulationStateMachine {
                    id: "BedMachine".to_string(),
                    label: "BedMachine".to_string(),
                    states: vec![state("bed.heating", true), state("bed.ready", false)],
                    transitions: vec![SimulationTransition {
                        id: "bed.ready".to_string(),
                        source: "bed.heating".to_string(),
                        target: "bed.ready".to_string(),
                        trigger: SimulationTrigger {
                            kind: SimulationTriggerKind::Completion,
                            value: None,
                        },
                        guard: None,
                        effects: vec![SimulationEffect::EmitSignal(SignalEffect {
                            signal_type: "BedReady".to_string(),
                            target: Some("printer".to_string()),
                        })],
                    }],
                },
                SimulationStateMachine {
                    id: "PrinterMachine".to_string(),
                    label: "PrinterMachine".to_string(),
                    states: vec![
                        state("printer.heating", true),
                        state("printer.printing", false),
                    ],
                    transitions: vec![SimulationTransition {
                        id: "printer.print".to_string(),
                        source: "printer.heating".to_string(),
                        target: "printer.printing".to_string(),
                        trigger: SimulationTrigger {
                            kind: SimulationTriggerKind::Signal,
                            value: Some("BedReady".to_string()),
                        },
                        guard: None,
                        effects: Vec::new(),
                    }],
                },
            ],
            derived_rules: Vec::new(),
            binding_rules: Vec::new(),
        };

        let trace = run_concurrent_simulation_model(
            &model,
            ConcurrentSimulationScenario {
                id: "scenario".to_string(),
                subjects: vec![
                    ConcurrentSubjectScenario {
                        subject_id: "bed".to_string(),
                        machine_id: "BedMachine".to_string(),
                        initial_state_id: None,
                        events: Vec::new(),
                    },
                    ConcurrentSubjectScenario {
                        subject_id: "printer".to_string(),
                        machine_id: "PrinterMachine".to_string(),
                        initial_state_id: None,
                        events: Vec::new(),
                    },
                ],
                max_steps: 6,
                step_duration_s: 1.0,
                clock_config: None,
                initial_values: BTreeMap::new(),
                requirements: Vec::new(),
                objectives: Vec::new(),
            },
            SimulationClockConfig::default(),
        )
        .unwrap();

        assert!(trace.timeline.iter().any(|entry| {
            entry
                .states
                .get("printer")
                .is_some_and(|states| states == &vec!["printer.printing".to_string()])
        }));
        assert!(trace.timeline.iter().any(|entry| {
            entry.events.iter().any(|event| {
                event.subject_id.as_deref() == Some("printer")
                    && event.trigger.as_deref() == Some("signal:bed:BedReady")
            })
        }));
    }

    fn state(id: &str, initial: bool) -> SimulationState {
        SimulationState {
            id: id.to_string(),
            label: id.to_string(),
            parent_state_id: None,
            is_initial: initial,
            is_final: false,
            is_orthogonal: false,
            is_history: false,
            entry_behavior: None,
            exit_behavior: None,
            do_behavior: None,
        }
    }

    fn transition(id: &str, source: &str, target: &str, trigger: &str) -> SimulationTransition {
        SimulationTransition {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            trigger: SimulationTrigger {
                kind: SimulationTriggerKind::Event,
                value: Some(trigger.to_string()),
            },
            guard: None,
            effects: Vec::new(),
        }
    }
}
