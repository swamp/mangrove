use crate::script::{compile, MangroveError};
use crate::util::{get_impl_func, get_impl_func_optional};
use limnus_gamepad::{Axis, GamepadMessage};
use std::cell::RefCell;
use std::rc::Rc;
use swamp::prelude::{App, Fp, LoReM, LocalResource, Msg, Plugin, UpdatePhase};
use swamp_advanced_game::ApplicationLogic;
use swamp_script::prelude::ModulePath;
use swamp_script_core::prelude::Value;
use swamp_script_eval::prelude::ExecuteError;
use swamp_script_eval::{util_execute_function, ExternalFunctions};
use swamp_script_semantic::{
    ResolvedInternalFunctionDefinitionRef, ResolvedModuleRef, ResolvedProgram,
};

pub fn logic_tick(mut script: LoReM<ScriptLogic>) {
    script.tick().expect("script.tick() crashed");
}

pub fn input_tick(mut script: LoReM<ScriptLogic>, gamepad_messages: Msg<GamepadMessage>) {
    for gamepad_message in gamepad_messages.iter_current() {
        script.gamepad(gamepad_message);
    }
}

#[derive(Debug)]
pub struct ScriptLogicContext {}

#[derive(LocalResource, Debug)]
pub struct ScriptLogic {
    logic_value_ref: Value,
    logic_fn: ResolvedInternalFunctionDefinitionRef,
    gamepad_axis_changed_fn: Option<ResolvedInternalFunctionDefinitionRef>,
    external_functions: ExternalFunctions<ScriptLogicContext>,
    script_context: ScriptLogicContext,
    resolved_program: ResolvedProgram,
}

impl ScriptLogic {
    pub fn new(
        logic_value_ref: Value,
        logic_fn: ResolvedInternalFunctionDefinitionRef,
        gamepad_changed_fn: Option<ResolvedInternalFunctionDefinitionRef>,
        external_functions: ExternalFunctions<ScriptLogicContext>,
        resolved_program: ResolvedProgram,
    ) -> Self {
        Self {
            logic_value_ref,
            logic_fn,
            gamepad_axis_changed_fn: gamepad_changed_fn,
            external_functions,
            script_context: ScriptLogicContext {},
            resolved_program,
        }
    }

    pub fn immutable_logic_value(&self) -> Value {
        match &self.logic_value_ref {
            Value::Reference(rc_refcell_value) => rc_refcell_value.borrow().clone(),
            _ => panic!("value should be reference in logic"),
        }
    }

    pub fn main_module(&self) -> &ResolvedModuleRef {
        let root_module_path = ModulePath(vec!["main".to_string()]);

        self.resolved_program
            .modules
            .get(&root_module_path)
            .expect("main module should exist in logic")
    }

    pub fn tick(&mut self) -> Result<(), ExecuteError> {
        let _ = util_execute_function(
            &self.external_functions,
            &self.logic_fn,
            &[self.logic_value_ref.clone()],
            &mut self.script_context,
        )?;

        Ok(())
    }

    fn execute(
        &mut self,
        fn_def: &ResolvedInternalFunctionDefinitionRef,
        arguments: &[Value],
    ) -> Result<(), ExecuteError> {
        let mut complete_arguments = Vec::new();
        complete_arguments.push(self.logic_value_ref.clone()); // push logic self first
        for arg in arguments {
            complete_arguments.push(arg.clone());
        }

        let _ = util_execute_function(
            &self.external_functions,
            fn_def,
            &complete_arguments,
            &mut self.script_context,
        )?;

        Ok(())
    }

    pub fn gamepad(&mut self, msg: &GamepadMessage) {
        match msg {
            GamepadMessage::Connected(_, _) => {}
            GamepadMessage::Disconnected(_) => {}
            GamepadMessage::Activated(_) => {}
            GamepadMessage::ButtonChanged(_, _, _) => {}
            GamepadMessage::AxisChanged(gamepad_id, axis, value) => {
                if *axis != Axis::LeftStickX {
                    return;
                }
                if let Some(found_fn) = &self.gamepad_axis_changed_fn {
                    let gamepad_id_value = Value::Int(*gamepad_id as i32);
                    let axis_value = Value::Float(Fp::from(*value));

                    let fn_ref = found_fn.clone(); // Assuming found_fn can be cloned

                    self.execute(&fn_ref, &[gamepad_id_value, axis_value])
                        .expect("gamepad_axis_changed");
                }
            }
        }
    }
}

pub fn boot() -> Result<ScriptLogic, MangroveError> {
    let mut resolved_program = ResolvedProgram::new();
    let mut external_functions = ExternalFunctions::<ScriptLogicContext>::new();

    compile(
        "scripts/logic.swamp".as_ref(),
        &mut resolved_program,
        &mut external_functions,
    )?;

    let root_module_path = ModulePath(vec!["main".to_string()]);
    let main_fn = {
        let main_module = resolved_program
            .modules
            .get(&root_module_path)
            .expect("could not find main module");

        let binding = main_module.borrow();
        let function_ref = binding
            .namespace
            .get_internal_function("main")
            .expect("No main function");

        Rc::clone(function_ref) // Clone the Rc, not the inner value
    };

    let mut script_context = ScriptLogicContext {};

    let logic_value =
        util_execute_function(&external_functions, &main_fn, &[], &mut script_context)?;

    let logic_struct_type_ref = if let Value::Struct(struct_type_ref, _, _) = &logic_value {
        struct_type_ref
    } else {
        return Err(MangroveError::Other("needs to be logic struct".to_string()));
    };

    let logic_fn = get_impl_func(&logic_struct_type_ref, "tick");
    let gamepad_changed_fn = get_impl_func_optional(&logic_struct_type_ref, "gamepad_axis_changed");

    // Convert it to a mutable (reference), so it can be mutated in update ticks
    let logic_value_ref = Value::Reference(Rc::new(RefCell::new(logic_value)));

    Ok(ScriptLogic::new(
        logic_value_ref,
        logic_fn,
        gamepad_changed_fn,
        external_functions,
        resolved_program,
    ))
}

pub struct ScriptLogicPlugin;

impl Plugin for ScriptLogicPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(UpdatePhase::Update, logic_tick);
        app.add_system(UpdatePhase::Update, input_tick);
        let script_logic = boot().expect("logic boot should work");
        app.insert_local_resource(script_logic);
    }
}