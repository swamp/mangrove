use crate::script::{compile, MangroveError};
use crate::util::{get_impl_func, get_impl_func_optional};
use crate::ScriptMessage;
use limnus_gamepad::{Axis, AxisValueType, Button, ButtonValueType, GamePadId, GamepadMessage};
use std::cell::RefCell;
use std::rc::Rc;
use swamp::prelude::{App, Fp, LoReM, LocalResource, Msg, Plugin, UpdatePhase};
use swamp_script::prelude::*;

use tracing::error;

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
    gamepad_button_changed_fn: Option<ResolvedInternalFunctionDefinitionRef>,
    external_functions: ExternalFunctions<ScriptLogicContext>,
    script_context: ScriptLogicContext,
    resolved_program: ResolvedProgram,
    //axis_enum_type: ResolvedEnumTypeRef,
    input_module: ResolvedModuleRef,
}

impl ScriptLogic {
    pub fn new(
        logic_value_ref: Value,
        logic_fn: ResolvedInternalFunctionDefinitionRef,
        gamepad_axis_changed_fn: Option<ResolvedInternalFunctionDefinitionRef>,
        gamepad_button_changed_fn: Option<ResolvedInternalFunctionDefinitionRef>,
        external_functions: ExternalFunctions<ScriptLogicContext>,
        resolved_program: ResolvedProgram,
        //axis_enum_type: ResolvedEnumTypeRef,
        input_module: ResolvedModuleRef,
    ) -> Self {
        Self {
            logic_value_ref,
            logic_fn,
            gamepad_axis_changed_fn,
            gamepad_button_changed_fn,
            external_functions,
            script_context: ScriptLogicContext {},
            resolved_program,
            //axis_enum_type,
            input_module,
        }
    }

    pub fn immutable_logic_value(&self) -> Value {
        match &self.logic_value_ref {
            Value::Reference(rc_refcell_value) => rc_refcell_value.borrow().clone(),
            _ => panic!("value should be reference in logic"),
        }
    }

    pub fn main_module(&self) -> &ResolvedModuleRef {
        let root_module_path = ModulePath(vec!["logic".to_string()]);

        self.resolved_program
            .modules
            .get(&root_module_path)
            .expect("logic module should exist in logic")
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
            GamepadMessage::ButtonChanged(gamepad_id, button, value) => {
                self.button_changed(gamepad_id, button, value)
            }
            GamepadMessage::AxisChanged(gamepad_id, axis, value) => {
                self.axis_changed(gamepad_id, axis, value)
            }
        }
    }

    fn axis_changed(&mut self, gamepad_id: &GamePadId, axis: &Axis, value: &AxisValueType) {
        let script_axis_value = {
            let input_module_ref = self.input_module.borrow();
            let axis_str = match axis {
                Axis::LeftStickX => "LeftStickX",
                Axis::LeftStickY => "LeftStickY",
                Axis::RightStickX => "RightStickX",
                Axis::RightStickY => "RightStickY",
            };

            let variant = input_module_ref
                .namespace
                .get_enum_variant_type_str("Axis", axis_str)
                .expect("axis");

            Value::EnumVariantSimple(variant.clone())
        };

        if let Some(found_fn) = &self.gamepad_axis_changed_fn {
            let gamepad_id_value = Value::Int(*gamepad_id as i32);
            let axis_value = Value::Float(Fp::from(*value));

            let fn_ref = found_fn.clone();

            self.execute(&fn_ref, &[gamepad_id_value, script_axis_value, axis_value])
                .expect("gamepad_axis_changed");
        }
    }

    fn button_changed(&mut self, gamepad_id: &GamePadId, button: &Button, value: &ButtonValueType) {
        let script_button_value = {
            let input_module_ref = self.input_module.borrow();
            let button_str = match button {
                Button::South => "South",
                Button::East => "East",
                Button::North => "North",
                Button::West => "West",
                Button::LeftTrigger => "LeftTrigger",
                Button::LeftTrigger2 => "LeftTrigger2",
                Button::RightTrigger => "RightTrigger",
                Button::RightTrigger2 => "RightTrigger2",
                Button::Select => "Select",
                Button::Start => "Start",
                Button::Mode => "Mode",
                Button::LeftThumb => "LeftThumb",
                Button::RightThumb => "RightThumb",
                Button::DPadUp => "DPadUp",
                Button::DPadDown => "DPadDown",
                Button::DPadLeft => "DPadLeft",
                Button::DPadRight => "DPadRight",
            };

            let variant = input_module_ref
                .namespace
                .get_enum_variant_type_str("Button", button_str)
                .expect("button name failed");

            Value::EnumVariantSimple(variant.clone())
        };

        if let Some(found_fn) = &self.gamepad_button_changed_fn {
            let gamepad_id_value = Value::Int(*gamepad_id as i32);
            let button_value = Value::Float(Fp::from(*value));

            let fn_ref = found_fn.clone();

            self.execute(
                &fn_ref,
                &[gamepad_id_value, script_button_value, button_value],
            )
            .expect("gamepad_button_changed");
        }
    }
}

pub fn input_module(
    resolve_state: &mut ResolvedProgramState,
) -> Result<(ResolvedModule, ResolvedEnumTypeRef, ResolvedEnumTypeRef), ResolveError> {
    let mut module = ResolvedModule::new(ModulePath(vec!["input".to_string()]));

    let axis_enum_type_ref = {
        let axis_enum_type_id = resolve_state.allocate_number(); // TODO: HACK
        let axis_enum_type_ref = module
            .namespace
            .create_enum_type(&LocalTypeIdentifier::from_str("Axis"), axis_enum_type_id)?;

        let names = ["LeftStickX", "LeftStickY", "RightStickX", "RightStickY"];
        for name in names {
            let variant_type_id = resolve_state.allocate_number(); // TODO: HACK
            let variant = ResolvedEnumVariantType::new(
                axis_enum_type_ref.clone(),
                LocalTypeIdentifier::from_str(name),
                ResolvedEnumVariantContainerType::Nothing,
                variant_type_id,
            );
            module.namespace.add_enum_variant(variant)?;
        }
        axis_enum_type_ref
    };

    let button_enum_type_ref = {
        let button_enum_type_id = resolve_state.allocate_number(); // TODO: HACK
        let button_enum_type_ref = module.namespace.create_enum_type(
            &LocalTypeIdentifier::from_str("Button"),
            button_enum_type_id,
        )?;

        let button_names = [
            "South",
            "East",
            "North",
            "West",
            "LeftTrigger",
            "LeftTrigger2",
            "RightTrigger",
            "RightTrigger2",
            "Select",
            "Start",
            "Mode",
            "LeftThumb",
            "RightThumb",
            "DPadUp",
            "DPadDown",
            "DPadLeft",
            "DPadRight",
        ];

        for button_name in button_names {
            let variant_type_id = resolve_state.allocate_number(); // TODO: HACK
            let variant = ResolvedEnumVariantType::new(
                button_enum_type_ref.clone(),
                LocalTypeIdentifier::from_str(button_name),
                ResolvedEnumVariantContainerType::Nothing,
                variant_type_id,
            );
            module.namespace.add_enum_variant(variant)?;
        }
        button_enum_type_ref
    };

    Ok((module, axis_enum_type_ref, button_enum_type_ref))
}

pub fn boot() -> Result<ScriptLogic, MangroveError> {
    let mut resolved_program = ResolvedProgram::new();
    let mut external_functions = ExternalFunctions::<ScriptLogicContext>::new();

    let (input_module, _axis_enum_type, _button_enum_type) =
        input_module(&mut resolved_program.state)?;
    let input_module_ref = Rc::new(RefCell::new(input_module));
    resolved_program
        .modules
        .add_module(input_module_ref.clone())?;

    compile(
        "scripts/logic.swamp".as_ref(),
        &mut resolved_program,
        &mut external_functions,
        "logic",
    )?;

    let root_module_path = ModulePath(vec!["logic".to_string()]);
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
    let gamepad_axis_changed_fn =
        get_impl_func_optional(&logic_struct_type_ref, "gamepad_axis_changed");
    let gamepad_button_changed_fn =
        get_impl_func_optional(&logic_struct_type_ref, "gamepad_button_changed");

    // Convert it to a mutable (reference), so it can be mutated in update ticks
    let logic_value_ref = Value::Reference(Rc::new(RefCell::new(logic_value)));

    Ok(ScriptLogic::new(
        logic_value_ref,
        logic_fn,
        gamepad_axis_changed_fn,
        gamepad_button_changed_fn,
        external_functions,
        resolved_program,
        // axis_enum_type,
        input_module_ref.clone(),
    ))
}

pub fn detect_reload_tick(
    script_messages: Msg<ScriptMessage>,
    mut script_logic: LoReM<ScriptLogic>,
) {
    for msg in script_messages.iter_previous() {
        match msg {
            ScriptMessage::Reload => match boot() {
                Ok(new_logic) => *script_logic = new_logic,
                Err(mangrove_error) => {
                    eprintln!("script logic failed: {}", mangrove_error);
                    error!(error=?mangrove_error, "script logic compile failed");
                }
            },
        }
    }
}

pub struct ScriptLogicPlugin;

impl Plugin for ScriptLogicPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(UpdatePhase::Update, detect_reload_tick);
        app.add_system(UpdatePhase::Update, logic_tick);
        app.add_system(UpdatePhase::Update, input_tick);

        let script_logic = boot().expect("logic boot should work");

        app.insert_local_resource(script_logic);
    }
}
