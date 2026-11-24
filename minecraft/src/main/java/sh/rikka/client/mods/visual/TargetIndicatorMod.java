package sh.rikka.client.mods.visual;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.rendertype.RenderTypes;
import net.minecraft.util.ARGB;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.Entity;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.ColorSetting;
import sh.rikka.client.setting.DoubleSetting;

public final class TargetIndicatorMod extends Mod {
	private static final int SEGMENTS = 48;
	private static final float MIN_RADIUS = 0.4F;
	private static final float RADIUS_PER_WIDTH = 0.75F;
	private static final float RADIANS_PER_TICK = 0.15F;
	private static final float TRAIL = 0.45F;
	private static final float LINE = 0.025F;
	private static final float TRAIL_ALPHA = 0.45F;
	private static volatile @Nullable TargetIndicatorMod active;

	private final ColorSetting color = color("color", 0xFF5EA8FF).opaque();
	private final DoubleSetting speed = number("speed", 1.0, 0.25, 3.0).step(0.05);
	private final DoubleSetting radius = number("radius", 1.0, 0.5, 2.0).step(0.05);
	private @Nullable EntityRenderState targetState;

	public TargetIndicatorMod() {
		super("target_indicator", Category.VISUAL, "adjust");
	}

	static @Nullable TargetIndicatorMod active() {
		return active;
	}

	@Override
	protected void onEnable() {
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
		this.targetState = null;
	}

	void extracted(Entity entity, EntityRenderState state) {
		if (RikkaClient.targets().target() == entity) {
			this.targetState = state;
		}
	}

	void submitted(EntityRenderState state, PoseStack poseStack, SubmitNodeCollector collector) {
		if (state != this.targetState) {
			return;
		}
		this.targetState = null;
		float ringRadius = Math.max(MIN_RADIUS, state.boundingBoxWidth * RADIUS_PER_WIDTH) * this.radius.getFloat();
		float phase = state.ageInTicks * RADIANS_PER_TICK * this.speed.getFloat();
		float height = state.boundingBoxHeight;
		float y = (Mth.sin(phase) + 1.0F) / 2.0F * height;
		float trail = -Mth.cos(phase) * TRAIL * Math.min(1.0F, height);
		int argb = this.color.argb();
		collector.submitCustomGeometry(poseStack, RenderTypes.debugQuads(), (pose, buffer) -> {
			band(pose, buffer, ringRadius, y, y + trail, ARGB.color(TRAIL_ALPHA, argb), ARGB.transparent(argb));
			band(pose, buffer, ringRadius, y - LINE, y + LINE, argb, argb);
		});
	}

	private static void band(PoseStack.Pose pose, VertexConsumer buffer, float radius, float from, float to, int fromColor, int toColor) {
		for (int i = 0; i < SEGMENTS; i++) {
			float a0 = Mth.TWO_PI * i / SEGMENTS;
			float a1 = Mth.TWO_PI * (i + 1) / SEGMENTS;
			float x0 = Mth.cos(a0) * radius;
			float z0 = Mth.sin(a0) * radius;
			float x1 = Mth.cos(a1) * radius;
			float z1 = Mth.sin(a1) * radius;
			buffer.addVertex(pose, x0, from, z0).setColor(fromColor);
			buffer.addVertex(pose, x0, to, z0).setColor(toColor);
			buffer.addVertex(pose, x1, to, z1).setColor(toColor);
			buffer.addVertex(pose, x1, from, z1).setColor(fromColor);
		}
	}
}
