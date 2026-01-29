use std::sync::Arc;

use gpui::{App, ElementId, Window};
use gpui_tesserae::components::select::SelectState;
use smol::lock::RwLock;

use crate::managers::Managers;

use super::models_menu::{
    ModelSelectItem, ModelSelection, OnModelItemClickFn, create_models_select_state,
    fetch_all_models, observe_providers_for_refresh,
};

pub struct ModelPicker {
    pub state: Arc<SelectState<ModelSelection, ModelSelectItem>>,
    pub has_no_providers: bool,
    pub has_no_model: bool,
}

impl ModelPicker {
    pub fn new(
        id: ElementId,
        managers: Arc<RwLock<Managers>>,
        fetch_on_create: bool,
        custom_on_item_click: Option<OnModelItemClickFn>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let models_select_state =
            create_models_select_state(id, managers.clone(), custom_on_item_click, window, cx);
        let state = Arc::new(models_select_state);

        let providers_entity = managers.read_blocking().models.providers.clone();
        observe_providers_for_refresh(&providers_entity, state.clone(), managers.clone(), cx);

        if fetch_on_create && state.items.read(cx).is_empty() {
            fetch_all_models(managers.clone(), state.clone(), cx);
        }

        let has_no_providers = providers_entity.read(cx).is_empty();
        let has_no_model = managers
            .read_blocking()
            .models
            .get_current_model(cx)
            .is_none();

        Self {
            state,
            has_no_providers,
            has_no_model,
        }
    }

    pub fn fetch_models_if_empty(&self, managers: Arc<RwLock<Managers>>, cx: &mut App) {
        if self.state.items.read(cx).is_empty() {
            fetch_all_models(managers, self.state.clone(), cx);
        }
    }
}
