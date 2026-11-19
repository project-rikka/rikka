package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import java.util.Arrays;
import net.minecraft.client.Minecraft;
import net.minecraft.server.packs.repository.RepositorySource;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.events.ClientHooks;
import sh.rikka.client.resource.ClientModPackSource;

@Mixin(Minecraft.class)
abstract class MinecraftMixin {
	@ModifyReturnValue(method = "createTitle", at = @At("RETURN"))
	private String rikka$createTitle(String title) {
		return title + " | " + RikkaClient.NAME;
	}

	@ModifyArg(
		method = "<init>",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/server/packs/repository/PackRepository;<init>([Lnet/minecraft/server/packs/repository/RepositorySource;)V"
		)
	)
	private RepositorySource[] rikka$addModPacks(RepositorySource[] sources) {
		RepositorySource[] extended = Arrays.copyOf(sources, sources.length + 1);
		extended[sources.length] = new ClientModPackSource();
		return extended;
	}

	@Inject(method = "tick", at = @At("HEAD"))
	private void rikka$tickPre(CallbackInfo callback) {
		ClientHooks.tickPre();
	}

	@Inject(method = "tick", at = @At("RETURN"))
	private void rikka$tickPost(CallbackInfo callback) {
		ClientHooks.tickPost();
	}

	@Inject(method = "close", at = @At("HEAD"))
	private void rikka$close(CallbackInfo callback) {
		RikkaClient.shutdown();
	}
}
