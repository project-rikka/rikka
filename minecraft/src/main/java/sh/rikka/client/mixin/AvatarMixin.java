package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import net.minecraft.world.entity.Avatar;
import net.minecraft.world.entity.HumanoidArm;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.mods.visual.VisualHooks;

@Mixin(Avatar.class)
abstract class AvatarMixin {
	@ModifyReturnValue(method = "getMainArm", at = @At("RETURN"))
	private HumanoidArm rikka$mainArm(HumanoidArm vanilla) {
		Avatar self = (Avatar) (Object) this;
		return VisualHooks.mainArm(self, vanilla);
	}
}
