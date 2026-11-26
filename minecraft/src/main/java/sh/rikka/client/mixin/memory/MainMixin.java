package sh.rikka.client.mixin.memory;

import java.util.Set;
import java.util.concurrent.CompletableFuture;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.main.Main;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.startup.ClassPreloader;
import sh.rikka.client.startup.ServerKeyPairs;

@Mixin(Main.class)
abstract class MainMixin {
	@Inject(method = "main", at = @At("HEAD"))
	private static void rikka$startBackgroundWork(String[] args, CallbackInfo callback) {
		ClassPreloader.start(Main.class.getClassLoader(), FabricLoader.getInstance().getGameDir().resolve(RikkaClient.MOD_ID));
		ServerKeyPairs.prepare();
	}

	@Redirect(
		method = "main",
		at = @At(value = "INVOKE", target = "Lnet/minecraft/util/datafix/DataFixers;optimize(Ljava/util/Set;)Ljava/util/concurrent/CompletableFuture;")
	)
	private static CompletableFuture<?> rikka$optimizeOnDemand(Set<?> types) {
		return CompletableFuture.completedFuture(null);
	}
}
