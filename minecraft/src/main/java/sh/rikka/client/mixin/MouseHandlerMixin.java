package sh.rikka.client.mixin;

import net.minecraft.client.Minecraft;
import net.minecraft.client.MouseHandler;
import net.minecraft.client.input.MouseButtonInfo;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.events.ClientHooks;

@Mixin(MouseHandler.class)
abstract class MouseHandlerMixin {
	@Shadow
	@Final
	private Minecraft minecraft;

	@Inject(method = "onButton", at = @At("HEAD"), cancellable = true)
	private void rikka$onButton(long handle, MouseButtonInfo buttonInfo, int action, CallbackInfo callback) {
		if (handle == this.minecraft.getWindow().handle() && ClientHooks.mouseInput(buttonInfo, action)) {
			callback.cancel();
		}
	}

	@Inject(method = "onScroll", at = @At("HEAD"), cancellable = true)
	private void rikka$onScroll(long handle, double horizontal, double vertical, CallbackInfo callback) {
		if (handle == this.minecraft.getWindow().handle() && ClientHooks.mouseScroll(horizontal, vertical)) {
			callback.cancel();
		}
	}
}
