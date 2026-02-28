use std::sync::Arc;

use gpui::{App, ElementId, Entity, SharedString, Window};
use gpui_tesserae::components::select::SelectState;

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
        managers: Managers,
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
        managers: Managers,
        models_cache: Entity<ModelsCache>,
        custom_on_item_click: Option<OnModelItemClickFn>,
        source: ModelSelectionSource,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let initial_selection: Option<InitialModelSelection> = managers.models.read_with(cx, |models, cx| {
            let pair = match source {
                ModelSelectionSource::Current => &models.current_model,
                ModelSelectionSource::ChatTitles => &models.chat_titles_model,
            };
            let pair = pair.read(cx);
            pair.as_ref().map(|p| {
                let icon_path: SharedString = models
                    .providers
                    .read(cx)
                    .get(&p.provider_id)
                    .map(|prov| prov.icon.read(cx).clone())
                    .unwrap_or_default();
                InitialModelSelection {
                    provider_id: p.provider_id.clone(),
                    provider_name: p.provider_name.clone(),
                    model_id: p.model.clone(),
                    parameters: p.parameters.clone(),
                    quantization: p.quantization.clone(),
                    icon_path,
                }
            })
        });

        let models_select_state = create_models_select_state(
            id,
            managers.clone(),
            custom_on_item_click,
            initial_selection,
            window,
            cx,
        );
        let state = Arc::new(models_select_state);

        let providers_entity = managers.models.read(cx).providers.clone();
        observe_providers_for_refresh(&providers_entity, state.clone(), managers.clone(), cx);

        let (current_provider_id, current_model) = managers.models.read_with(cx, |models, cx| {
            let pair = match source {
                ModelSelectionSource::Current => &models.current_model,
                ModelSelectionSource::ChatTitles => &models.chat_titles_model,
            };
            match pair.read(cx).as_ref() {
                Some(p) => (Some(p.provider_id.clone()), Some(p.model.clone())),
                None => (None, None),
            }
        });

        populate_state_from_cache(
            &state,
            &models_cache,
            current_provider_id.as_ref(),
            current_model.as_ref(),
            cx,
        );

        {
            let state = state.clone();
            let managers = managers.clone();
            cx.observe(&models_cache, move |models_cache, cx| {
                state.items.update(cx, |items, cx| {
                    *items = gpui_tesserae::components::select::SelectItemsMap::new();
                    cx.notify();
                });

                let (current_provider_id, current_model) = managers.models.read_with(cx, |models, cx| {
                    let pair = match source {
                        ModelSelectionSource::Current => &models.current_model,
                        ModelSelectionSource::ChatTitles => &models.chat_titles_model,
                    };
                    match pair.read(cx).as_ref() {
                        Some(p) => (Some(p.provider_id.clone()), Some(p.model.clone())),
                        None => (None, None),
                    }
                });

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
        let has_no_model = managers.models.read_with(cx, |models, cx| {
            match source {
                ModelSelectionSource::Current => models.get_current_model(cx).is_none(),
                ModelSelectionSource::ChatTitles => {
                    models.get_chat_titles_model(cx).is_none()
                }
            }
        });

        Self {
            state,
            models_cache,
            has_no_providers,
            has_no_model,
        }
    }
}
