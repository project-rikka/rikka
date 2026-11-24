package sh.rikka.client.mixin;

import com.mojang.blaze3d.platform.Window;
import org.lwjgl.sdl.SDLEvents;
import org.lwjgl.sdl.SDL_Event;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.mods.motionblur.MotionBlurHooks;

@Mixin(Window.class)
abstract class WindowMixin {
	@Inject(method = "handleEvent", at = @At("HEAD"))
	private void rikka$handleEvent(SDL_Event event, CallbackInfo callback) {
		switch (event.type()) {
			case SDLEvents.SDL_EVENT_DISPLAY_CURRENT_MODE_CHANGED,
				SDLEvents.SDL_EVENT_WINDOW_DISPLAY_CHANGED,
				SDLEvents.SDL_EVENT_WINDOW_ENTER_FULLSCREEN,
				SDLEvents.SDL_EVENT_WINDOW_LEAVE_FULLSCREEN -> MotionBlurHooks.displayModeChanged();
			default -> {
			}
		}
	}
}
