package sh.rikka.client.mods.visual;

import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.Camera;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.player.RemotePlayer;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.entity.state.TntRenderState;
import net.minecraft.world.entity.Avatar;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.HumanoidArm;
import net.minecraft.world.entity.item.PrimedTnt;

public final class VisualHooks {
	private VisualHooks() {
	}

	public static float lightmapBrightness(float vanilla) {
		FullbrightMod fullbright = FullbrightMod.active();
		return fullbright == null ? vanilla : fullbright.brightness(vanilla);
	}

	public static HumanoidArm mainArm(Avatar avatar, HumanoidArm vanilla) {
		ForceMainHandMod forceMainHand = ForceMainHandMod.active();
		return forceMainHand != null && avatar instanceof RemotePlayer ? forceMainHand.arm() : vanilla;
	}

	public static boolean crosshairShown(boolean firstPerson) {
		CrosshairMod crosshair = CrosshairMod.active();
		return firstPerson || crosshair != null && crosshair.shownInThirdPerson();
	}

	public static boolean drawCrosshair(GuiGraphicsExtractor graphics) {
		CrosshairMod crosshair = CrosshairMod.active();
		return crosshair != null && crosshair.draw(graphics);
	}

	public static long overworldClockTicks(long vanilla) {
		TimeChangerMod timeChanger = TimeChangerMod.active();
		return timeChanger == null ? vanilla : timeChanger.clockTicks(vanilla);
	}

	public static void entityExtracted(Entity entity, EntityRenderState state) {
		TargetIndicatorMod indicator = TargetIndicatorMod.active();
		if (indicator != null) {
			indicator.extracted(entity, state);
		}
		TntTimerMod tntTimer = TntTimerMod.active();
		if (tntTimer != null && entity instanceof PrimedTnt tnt && state instanceof TntRenderState tntState) {
			tntTimer.extracted(tnt, tntState);
		}
	}

	public static boolean labelsPrimedTnt() {
		return TntTimerMod.active() != null;
	}

	public static void entitySubmitted(EntityRenderState state, PoseStack poseStack, SubmitNodeCollector collector) {
		TargetIndicatorMod indicator = TargetIndicatorMod.active();
		if (indicator != null) {
			indicator.submitted(state, poseStack, collector);
		}
	}

	public static void emitGizmos(Camera camera) {
		ChunkBordersMod chunkBorders = ChunkBordersMod.active();
		if (chunkBorders != null) {
			chunkBorders.emitGizmos(camera);
		}
	}
}
