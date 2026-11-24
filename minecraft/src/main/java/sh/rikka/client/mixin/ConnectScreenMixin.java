package sh.rikka.client.mixin;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.ConnectScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.client.multiplayer.TransferState;
import net.minecraft.client.multiplayer.resolver.ServerAddress;
import net.minecraft.network.Connection;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.events.ClientHooks;

@Mixin(ConnectScreen.class)
abstract class ConnectScreenMixin extends Screen {
	@Shadow
	private volatile @Nullable Connection connection;

	@Shadow
	private volatile boolean aborted;

	private ConnectScreenMixin(Component title) {
		super(title);
	}

	@Inject(method = "startConnecting", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/Minecraft;prepareForMultiplayer()V"))
	private static void rikka$connect(
		Screen parent, Minecraft minecraft, ServerAddress address, ServerData data, boolean quickPlay, @Nullable TransferState transfer, CallbackInfo callback
	) {
		ClientHooks.serverConnect(address, transfer != null);
	}

	@Override
	public void removed() {
		super.removed();
		Connection current = this.connection;
		if (this.aborted || current == null || !current.isConnected()) {
			ClientHooks.disconnected();
		}
	}
}
