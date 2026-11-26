package sh.rikka.client.mixin.memory;

import java.security.KeyPair;
import net.minecraft.server.MinecraftServer;
import net.minecraft.util.CryptException;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;
import sh.rikka.client.startup.ServerKeyPairs;

@Mixin(MinecraftServer.class)
abstract class MinecraftServerMixin {
	@Redirect(method = "initializeKeyPair", at = @At(value = "INVOKE", target = "Lnet/minecraft/util/Crypt;generateKeyPair()Ljava/security/KeyPair;"))
	private KeyPair rikka$pregeneratedKeyPair() throws CryptException {
		return ServerKeyPairs.take();
	}
}
