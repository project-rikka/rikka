package sh.rikka.client.mixin;

import net.minecraft.client.gui.screens.ChatScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.Style;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
import sh.rikka.client.mods.translate.ChatTranslateHooks;
import sh.rikka.client.mods.translate.TranslateBar;

@Mixin(ChatScreen.class)
abstract class ChatScreenMixin extends Screen {
	@Unique
	private @Nullable TranslateBar rikka$translateBar;

	private ChatScreenMixin(Component title) {
		super(title);
	}

	@Shadow
	public abstract String normalizeChatMessage(String message);

	@Inject(method = "init", at = @At("TAIL"))
	private void rikka$addTranslateBar(CallbackInfo callback) {
		TranslateBar bar = ChatTranslateHooks.bar(this.font, this.width, this.height).orElse(null);
		this.rikka$translateBar = bar == null ? null : addRenderableWidget(bar);
	}

	@Inject(method = "mouseScrolled", at = @At("HEAD"), cancellable = true)
	private void rikka$scrollTranslateBar(double x, double y, double scrollX, double scrollY, CallbackInfoReturnable<Boolean> callback) {
		TranslateBar bar = this.rikka$translateBar;
		if (bar != null && bar.scrolled(x, y, scrollY)) {
			callback.setReturnValue(true);
		}
	}

	@Inject(method = "handleComponentClicked", at = @At("HEAD"), cancellable = true)
	private void rikka$clickTranslateButton(Style clicked, boolean allowInsertions, CallbackInfoReturnable<Boolean> callback) {
		if (!allowInsertions && ChatTranslateHooks.click(clicked.getClickEvent())) {
			callback.setReturnValue(true);
		}
	}

	@Inject(method = "handleChatInput", at = @At("HEAD"), cancellable = true)
	private void rikka$sendTranslated(String message, boolean addToRecent, CallbackInfo callback) {
		if (ChatTranslateHooks.send(normalizeChatMessage(message), addToRecent)) {
			callback.cancel();
		}
	}
}
