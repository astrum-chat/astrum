use std::sync::Arc;

use gpui::{App, ElementId, Entity, Window};
use gpui_tesserae::components::select::SelectState;
use smol::lock::RwLock;

use crate::managers::Managers;

use super::models_menu::{
    InitialModelSelection, ModelSelectItem, ModelSelection, ModelSelectionSource, ModelsCache,
    OnModelItemClickFn, create_models_select_state, observe_providers_for_refresh,
    populate_state_from_cache,
};

pub struct ModelPicker {
    pub state: Arc<SelectState<ModelSelection, ModelSelectItem>>,
    pub models_cache: Entity<ModelsCache>,
    pub has_no_providers: bool,
    pub has_no_model: bool,
}

impl ModelPicker {
    pub fn new(
        id: ElementId,
        managers: Arc<RwLock<Managers>>,
        models_cache: Entity<ModelsCache>,
        custom_on_item_click: Option<OnModelItemClickFn>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        Self::new_with_source(
            id,
            managers,
            models_cache,
            custom_on_item_click,
            ModelSelectionSource::Current,
            window,
            cx,
        )
    }

    pub fn new_with_source(
        id: ElementId,
        managers: Arc<RwLock<Managers>>,
        models_cache: Entity<ModelsCache>,
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

        // Get current selection info for populating state
        let (current_provider_id, current_model) = {
            let managers = managers.read_blocking();
            match source {
                ModelSelectionSource::Current => (
                    managers.models.current_model.provider_id.read(cx).clone(),
                    managers.models.get_current_model(cx).cloned(),
                ),
                ModelSelectionSource::ChatTitles => (
                    managers
                        .models
                        .chat_titles_model
                        .provider_id
                        .read(cx)
                        .clone(),
                    managers.models.get_chat_titles_model(cx).cloned(),
                ),
            }
        };

        // Populate state from cache initially
        populate_state_from_cache(
            &state,
            &models_cache,
            current_provider_id.as_ref(),
            current_model.as_ref(),
            cx,
        );

        // Observe cache changes and repopulate state
        {
            let state = state.clone();
            let managers = managers.clone();
            cx.observe(&models_cache, move |models_cache, cx| {
                // Clear existing items by replacing with new empty map
                state.items.update(cx, |items, cx| {
                    *items = gpui_tesserae::components::select::SelectItemsMap::new();
                    cx.notify();
                });

                // Get current selection info
                let (current_provider_id, current_model) = {
                    let managers = managers.read_blocking();
                    match source {
                        ModelSelectionSource::Current => (
                            managers.models.current_model.provider_id.read(cx).clone(),
                            managers.models.get_current_model(cx).cloned(),
                        ),
                        ModelSelectionSource::ChatTitles => (
                            managers
                                .models
                                .chat_titles_model
                                .provider_id
                                .read(cx)
                                .clone(),
                            managers.models.get_chat_titles_model(cx).cloned(),
                        ),
                    }
                };

                // Repopulate from cache
                populate_state_from_cache(
                    &state,
                    &models_cache,
                    current_provider_id.as_ref(),
                    current_model.as_ref(),
                    cx,
                );
            })
            .detach();
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
            models_cache,
            has_no_providers,
            has_no_model,
        }
    }
}
