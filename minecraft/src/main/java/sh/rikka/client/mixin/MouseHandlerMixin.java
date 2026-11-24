package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.Minecraft;
import net.minecraft.client.MouseHandler;
import net.minecraft.client.input.MouseButtonInfo;
import net.minecraft.client.player.LocalPlayer;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.events.ClientHooks;
import sh.rikka.client.mods.camera.CameraHooks;

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

	@WrapOperation(
		method = "turnPlayer",
		at = @At(value = "INVOKE", target = "Lnet/minecraft/client/player/LocalPlayer;turn(DD)V")
	)
	private void rikka$turn(LocalPlayer player, double horizontal, double vertical, Operation<Void> original) {
		ClientHooks.look(horizontal, vertical);
		double scale = CameraHooks.turnScale();
		if (!CameraHooks.freeLookTurn(player, horizontal * scale, vertical * scale)) {
			original.call(player, horizontal * scale, vertical * scale);
		}
	}
}
