use std::sync::Arc;

use gpui::{App, ElementId, Window};
use gpui_tesserae::components::select::SelectState;
use smol::lock::RwLock;

use crate::managers::Managers;

use super::models_menu::{
    InitialModelSelection, ModelSelectItem, ModelSelection, ModelSelectionSource,
    OnModelItemClickFn, create_models_select_state, fetch_all_models_with_source,
    observe_providers_for_refresh,
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
        Self::new_with_source(
            id,
            managers,
            fetch_on_create,
            custom_on_item_click,
            ModelSelectionSource::Current,
            window,
            cx,
        )
    }

    pub fn new_with_source(
        id: ElementId,
        managers: Arc<RwLock<Managers>>,
        fetch_on_create: bool,
        custom_on_item_click: Option<OnModelItemClickFn>,
        source: ModelSelectionSource,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        // Compute initial selection from stored provider_id, provider_name, and model
        let initial_selection: Option<InitialModelSelection> = {
            let managers = managers.read_blocking();
            let (provider_id, provider_name, model) = match source {
                ModelSelectionSource::Current => (
                    managers.models.current_model.provider_id.read(cx).clone(),
                    managers.models.current_model.provider_name.read(cx).clone(),
                    managers.models.current_model.model.read(cx).clone(),
                ),
                ModelSelectionSource::ChatTitles => (
                    managers
                        .models
                        .chat_titles_model
                        .provider_id
                        .read(cx)
                        .clone(),
                    managers
                        .models
                        .chat_titles_model
                        .provider_name
                        .read(cx)
                        .clone(),
                    managers.models.chat_titles_model.model.read(cx).clone(),
                ),
            };
            match (provider_id, provider_name, model) {
                (Some(pid), Some(pn), Some(m)) => Some(InitialModelSelection {
                    provider_id: pid,
                    provider_name: pn,
                    model_id: m,
                }),
                _ => None,
            }
        };

        let models_select_state = create_models_select_state(
            id,
            managers.clone(),
            custom_on_item_click,
            initial_selection,
            window,
            cx,
        );
        let state = Arc::new(models_select_state);

        let providers_entity = managers.read_blocking().models.providers.clone();
        observe_providers_for_refresh(&providers_entity, state.clone(), managers.clone(), cx);

        // <=1 accounts for placeholder item
        if fetch_on_create && state.items.read(cx).len() <= 1 {
            fetch_all_models_with_source(managers.clone(), state.clone(), source, cx);
        }

        let has_no_providers = providers_entity.read(cx).is_empty();
        let has_no_model = {
            let managers = managers.read_blocking();
            match source {
                ModelSelectionSource::Current => managers.models.get_current_model(cx).is_none(),
                ModelSelectionSource::ChatTitles => {
                    managers.models.get_chat_titles_model(cx).is_none()
                }
            }
        };

        Self {
            state,
            has_no_providers,
            has_no_model,
        }
    }

    pub fn fetch_models_if_empty(&self, managers: Arc<RwLock<Managers>>, cx: &mut App) {
        self.fetch_models_if_empty_with_source(managers, ModelSelectionSource::Current, cx);
    }

    pub fn fetch_models_if_empty_with_source(
        &self,
        managers: Arc<RwLock<Managers>>,
        source: ModelSelectionSource,
        cx: &mut App,
    ) {
        // <=1 accounts for placeholder item
        if self.state.items.read(cx).len() <= 1 {
            fetch_all_models_with_source(managers, self.state.clone(), source, cx);
        }
    }
}
