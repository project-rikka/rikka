package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import com.mojang.authlib.GameProfile;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.world.entity.player.PlayerSkin;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.mods.privacy.PrivacyHooks;

@Mixin(PlayerInfo.class)
abstract class PlayerInfoMixin {
	@Shadow
	public abstract GameProfile getProfile();

	@ModifyReturnValue(method = "getSkin", at = @At("RETURN"))
	private PlayerSkin rikka$protectSkin(PlayerSkin skin) {
		return PrivacyHooks.skin(getProfile(), skin);
	}
}
