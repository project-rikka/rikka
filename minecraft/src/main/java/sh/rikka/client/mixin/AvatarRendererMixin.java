package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import net.minecraft.client.model.HumanoidModel;
import net.minecraft.client.renderer.entity.player.AvatarRenderer;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.Avatar;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.mods.animation.AnimationHooks;

@Mixin(AvatarRenderer.class)
abstract class AvatarRendererMixin {
	@ModifyReturnValue(
		method = "getArmPose(Lnet/minecraft/world/entity/Avatar;Lnet/minecraft/world/item/ItemStack;Lnet/minecraft/world/InteractionHand;)Lnet/minecraft/client/model/HumanoidModel$ArmPose;",
		at = @At("RETURN")
	)
	private static HumanoidModel.ArmPose rikka$armPose(HumanoidModel.ArmPose pose, Avatar avatar, ItemStack itemInHand, InteractionHand hand) {
		return AnimationHooks.armPose(pose, avatar, hand);
	}
}
