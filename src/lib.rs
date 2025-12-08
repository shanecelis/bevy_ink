use bevy::prelude::*;
use bevy::asset::{AssetLoader, AsyncReadExt, LoadContext, LoadedAsset, io::Reader};
use bevy::reflect::TypeRegistry;
use bladeink::{story::Story, story_error::StoryError};
#[cfg(feature = "scripting")]
use bevy_mod_scripting::{
    GetTypeDependencies,
    lua::mlua::UserData,
    bindings::{
        InteropError,
        docgen::typed_through::{ThroughTypeInfo, TypedThrough},
        script_value::ScriptValue,
        function::from::FromScript,
        IntoScript,
        WorldAccessGuard,
    }
};
use std::any::TypeId;

pub struct InkPlugin;

impl Plugin for InkPlugin {
    fn build(&self, app: &mut App) {
        app
            .register_type::<InkStoryRef>()
            .init_non_send_resource::<InkStories>()
            .init_asset::<InkText>()
            .init_asset_loader::<InkTextLoader>();
        #[cfg(feature = "scripting")]
        lua::plugin(app);
    }
}


#[derive(Default)]
struct InkStories(Vec<InkStory>);

impl InkStories {
    fn insert(&mut self, handle: Handle<InkText>) -> InkStoryRef {
        self.0.push(InkStory::Unloaded(handle));
        InkStoryRef { index: self.0.len() - 1 }
    }
}
        

enum InkStory {
    Unloaded(Handle<InkText>),
    Loaded(Result<Story, StoryError>),
}

#[derive(Debug, Clone, Copy, Reflect)]
#[cfg_attr(feature = "scripting", derive(GetTypeDependencies))]
struct InkStoryRef { index: usize }

#[cfg(feature = "scripting")]
impl IntoScript for InkStoryRef {
    fn into_script(self, _world: WorldAccessGuard<'_>) -> Result<ScriptValue, InteropError> {
        Ok(ScriptValue::Integer(self.index as i64))
    }
}

#[cfg(feature = "scripting")]
impl FromScript for InkStoryRef {
    type This<'w> = Self;
    fn from_script(value: ScriptValue, _world: WorldAccessGuard<'_>) -> Result<Self::This<'_>, InteropError> {
        match value {
            ScriptValue::Integer(n) => Ok(InkStoryRef { index: n as usize }),
            x => Err(InteropError::value_mismatch(TypeId::of::<i64>(), x)),
        }
    }
}

#[cfg(feature = "scripting")]
impl TypedThrough for InkStoryRef {
    fn through_type_info() -> ThroughTypeInfo {
        ThroughTypeInfo::TypeInfo(<InkStoryRef as bevy::reflect::Typed>::type_info())
    }
}

#[cfg(feature = "scripting")]
impl UserData for InkStoryRef {}

#[derive(Asset, TypePath)]
pub struct InkText(pub String);

#[derive(Default)]
pub struct InkTextLoader;

#[derive(Event)]
pub enum InkEvent {
    Load(Handle<InkText>),
}

impl AssetLoader for InkTextLoader {
    type Asset = InkText;
    type Settings = ();
    type Error = std::io::Error;

    fn extensions(&self) -> &[&str] {
        &["txt"]
    }

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(InkText(String::from_utf8_lossy(&bytes).into()))
    }
}

#[cfg(feature = "scripting")]
mod lua {
    use super::*;

    use bevy_mod_scripting::bindings::{
        ReflectAccessId,
        function::{
        namespace::{GlobalNamespace, NamespaceBuilder},
        script_function::FunctionCallContext,
    }};
    pub(crate) fn plugin(app: &mut App) {
        let world = app.world_mut();

        NamespaceBuilder::<GlobalNamespace>::new_unregistered(world).register(
            "ink_load",
            |ctx: FunctionCallContext,
             path: String| {
                 let world_guard = ctx.world()?;
                 let raid = ReflectAccessId::for_global();
                 if world_guard.claim_global_access() {
                     let world = world_guard.as_unsafe_world_cell()?;
                     let world = unsafe { world.world_mut() };
                     let ink_text = {
                         let asset_server = world.resource::<AssetServer>();
                         asset_server.load::<InkText>(&path)
                     };
                     let mut ink_stories = world.non_send_resource_mut::<InkStories>();
                     let ink_story_ref = ink_stories.insert(ink_text);
                     Ok(ink_story_ref)
                 } else {
                     Err(InteropError::cannot_claim_access(
                         raid,
                         world_guard.get_access_location(raid),
                         "with_system_param",
                     ))
                 }
            },
        );
    }
}
