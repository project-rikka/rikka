use std::sync::Arc;

use windows::Foundation::TypedEventHandler;
use windows::UI::ViewManagement::{AccessibilitySettings, UISettings};
use windows::core::{IInspectable, Ref, Result};

type Changed = Arc<dyn Fn() + Send + Sync>;

#[must_use = "the watch stops when dropped"]
pub struct AppearanceWatch {
    colors: UISettings,
    colors_token: i64,
    contrast: AccessibilitySettings,
    contrast_token: i64,
}

impl AppearanceWatch {
    pub fn new(changed: impl Fn() + Send + Sync + 'static) -> Result<AppearanceWatch> {
        let changed: Changed = Arc::new(changed);
        let colors = UISettings::new()?;
        let colors_token =
            colors.ColorValuesChanged(&TypedEventHandler::new(notify::<UISettings>(
                changed.clone(),
            )))?;
        let contrast = AccessibilitySettings::new()?;
        let contrast_token =
            contrast.HighContrastChanged(&TypedEventHandler::new(notify::<AccessibilitySettings>(
                changed,
            )));
        let contrast_token = match contrast_token {
            Ok(token) => token,
            Err(error) => {
                if let Err(error) = colors.RemoveColorValuesChanged(colors_token) {
                    tracing::warn!("failed to stop watching system colours: {error}");
                }
                return Err(error);
            }
        };
        Ok(AppearanceWatch {
            colors,
            colors_token,
            contrast,
            contrast_token,
        })
    }
}

fn notify<S>(
    changed: Changed,
) -> impl Fn(Ref<'_, S>, Ref<'_, IInspectable>) -> Result<()> + Send + 'static
where
    S: windows::core::RuntimeType + 'static,
{
    move |_, _| {
        changed();
        Ok(())
    }
}

impl Drop for AppearanceWatch {
    fn drop(&mut self) {
        if let Err(error) = self.colors.RemoveColorValuesChanged(self.colors_token) {
            tracing::warn!("failed to stop watching system colours: {error}");
        }
        if let Err(error) = self.contrast.RemoveHighContrastChanged(self.contrast_token) {
            tracing::warn!("failed to stop watching high contrast: {error}");
        }
    }
}

impl std::fmt::Debug for AppearanceWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppearanceWatch").finish_non_exhaustive()
    }
}
