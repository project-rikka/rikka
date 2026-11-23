package sh.rikka.client.mods.animation;

import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.core.BlockPos;
import net.minecraft.core.component.DataComponents;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.HitResult;

final class BlockPunch {
	private BlockPunch() {
	}

	static void tick(Minecraft minecraft) {
		if (minecraft.level == null || minecraft.gameMode == null || !HeldInput.held(minecraft, minecraft.options.keyAttack)) {
			return;
		}
		LocalPlayer player = minecraft.player;
		if (player.isSpectator() || player.getMainHandItem().has(DataComponents.PIERCING_WEAPON)) {
			return;
		}
		if (!(minecraft.hitResult instanceof BlockHitResult hit) || hit.getType() != HitResult.Type.BLOCK) {
			return;
		}
		boolean vanillaSwings = minecraft.gameMode.isDestroying() && !player.isUsingItem();
		BlockPos position = hit.getBlockPos();
		if (vanillaSwings || minecraft.level.getBlockState(position).isAir()) {
			return;
		}
		if (player.swing(InteractionHand.MAIN_HAND, player.getMainHandItem().getAttackAnimation(), false)) {
			minecraft.level.addBreakingBlockEffects(position, hit.getDirection(), false);
		}
	}
}
