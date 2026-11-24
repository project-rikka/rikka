package sh.rikka.client.mods.animation;

import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.client.renderer.state.level.FirstPersonHandsAndItemsRenderState;
import net.minecraft.client.renderer.state.level.PlayerRenderState;
import net.minecraft.client.renderer.texture.OverlayTexture;
import net.minecraft.core.component.DataComponents;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.HumanoidArm;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import org.joml.Matrix4f;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mods.animation.FirstPersonPose.Use;

final class FirstPersonHand {
	private FirstPersonHand() {
	}

	static boolean submit(
		OldAnimationsMod mod,
		PlayerRenderState playerState,
		FirstPersonHandsAndItemsRenderState state,
		float partialTicks,
		InteractionHand hand,
		float attack,
		ItemStack stack,
		float inverseArmHeight,
		PoseStack poseStack,
		SubmitNodeCollector collector,
		int lightCoords
	) {
		AvatarRenderState avatar = playerState.avatarRenderState;
		if (avatar == null || hand != InteractionHand.MAIN_HAND || state.isScoping || stack.isEmpty()) {
			return false;
		}
		if (stack.has(DataComponents.MAP_ID) || stack.is(Items.CROSSBOW)) {
			return false;
		}
		boolean using = avatar.isUsingItem && state.useItemRemainingTicks > 0 && avatar.useItemHand == hand;
		Use use = use(mod, avatar, stack, using);
		if (use == null) {
			return false;
		}
		boolean block = use == Use.BLOCK;
		FirstPersonPose.Frame frame = new FirstPersonPose.Frame(
			use,
			using || block,
			block ? mod.blockHit() : !using || mod.swingWhileUsing(),
			partialTicks,
			inverseArmHeight,
			attack,
			state.useItemRemainingTicks,
			state.mainHandUseDuration
		);
		Matrix4f placement = placement(mod, avatar, state, frame);
		if (placement == null) {
			return false;
		}
		poseStack.pushPose();
		poseStack.mulPose(placement);
		state.mainHandRenderState.submit(poseStack, collector, lightCoords, OverlayTexture.NO_OVERLAY, 0);
		poseStack.popPose();
		return true;
	}

	private static @Nullable Use use(OldAnimationsMod mod, AvatarRenderState avatar, ItemStack stack, boolean using) {
		if (using) {
			return switch (stack.getUseAnimation()) {
				case EAT, DRINK -> Use.EAT;
				case BOW -> Use.BOW;
				case NONE -> Use.PLAIN;
				case BLOCK -> HeldItems.isShield(stack) ? null : Use.BLOCK;
				default -> null;
			};
		}
		LocalPlayer player = Minecraft.getInstance().player;
		if (player != null && mod.blocking().isBlocking(player)) {
			return Use.BLOCK;
		}
		return avatar.isAutoSpinAttack || HeldItems.isStabbing(avatar.currentSwing) ? null : Use.PLAIN;
	}

	private static @Nullable Matrix4f placement(
		OldAnimationsMod mod, AvatarRenderState avatar, FirstPersonHandsAndItemsRenderState state, FirstPersonPose.Frame frame
	) {
		boolean block = frame.use() == Use.BLOCK;
		Matrix4f chain = FirstPersonPose.chain(frame);
		if (avatar.mainArm == HumanoidArm.LEFT) {
			return block ? SpritePlacement.mirrored(FirstPersonPose.approximate(chain)) : null;
		}
		DisplayedItem item = mod.firstPersonItems() ? DisplayedItem.of(state.mainHandRenderState, ItemDisplayContext.FIRST_PERSON_RIGHT_HAND) : null;
		if (item != null) {
			return FirstPersonPose.exact(chain, item.family(), item.display());
		}
		return block ? FirstPersonPose.approximate(chain) : null;
	}
}
