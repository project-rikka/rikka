package sh.rikka.client.mods.visual;

import com.mojang.blaze3d.platform.Window;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.item.BowItem;
import net.minecraft.world.item.CrossbowItem;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.phys.Vec3;
import org.joml.Matrix3x2fStack;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.render.GuiMesh;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ColorSetting;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.ui.Smoothed;

public final class CrosshairMod extends Mod {
	private static final float SPREAD_MILLIS = 60.0F;
	private static final float SPREAD_EPSILON = 0.005F;
	private static final double SPRINT_SPEED = 0.28;
	private static final float MOVEMENT_SPREAD = 0.6F;
	private static final float AIRBORNE_SPREAD = 0.4F;
	private static final float COOLDOWN_SPREAD = 0.5F;
	private static final float FEATHER = 1.0F;
	private static final float DIAGONAL = Mth.SQRT_OF_TWO / 2.0F;
	private static volatile @Nullable CrosshairMod active;

	private final EnumSetting<CrosshairStyle> style = choice("style", CrosshairStyle.CROSS);
	private final EnumSetting<CrosshairBlend> blend = choice("blend", CrosshairBlend.NORMAL).visibleWhen(this::isCustom);
	private final ColorSetting color = color("color", 0xFFFFFFFF).visibleWhen(this::isCustom);
	private final DoubleSetting length = number("length", 4.0, 0.5, 20.0).step(0.5).visibleWhen(this::hasArms);
	private final DoubleSetting thickness = number("thickness", 1.0, 0.5, 5.0).step(0.5).visibleWhen(this::isCustom);
	private final DoubleSetting gap = number("gap", 2.0, 0.0, 12.0).step(0.5).visibleWhen(this::hasGap);
	private final BoolSetting dot = bool("dot", false).visibleWhen(() -> isCustom() && !this.style.is(CrosshairStyle.DOT));
	private final BoolSetting outline = bool("outline", true).visibleWhen(this::isOutlinable);
	private final ColorSetting outlineColor = color("outline_color", 0xC0000000).visibleWhen(() -> isOutlinable() && this.outline.get());
	private final DoubleSetting outlineThickness = number("outline_thickness", 0.5, 0.5, 3.0)
		.step(0.5)
		.visibleWhen(() -> isOutlinable() && this.outline.get());
	private final BoolSetting dynamic = bool("dynamic", true).visibleWhen(this::hasGap);
	private final DoubleSetting dynamicSpread = number("dynamic_spread", 3.0, 0.5, 12.0)
		.step(0.5)
		.visibleWhen(() -> hasGap() && this.dynamic.get());
	private final BoolSetting highlight = bool("highlight", true).visibleWhen(this::isCustom);
	private final ColorSetting highlightColor = color("highlight_color", 0xFFFF4D4D).visibleWhen(() -> isCustom() && this.highlight.get());
	private final BoolSetting thirdPerson = bool("third_person", false);
	private final Smoothed spread = new Smoothed(SPREAD_MILLIS, SPREAD_EPSILON, 0.0F);

	public CrosshairMod() {
		super("crosshair", Category.VISUAL, "point_scan");
	}

	static @Nullable CrosshairMod active() {
		return active;
	}

	@Override
	protected void onEnable() {
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
	}

	boolean shownInThirdPerson() {
		return this.thirdPerson.get();
	}

	boolean draw(GuiGraphicsExtractor graphics) {
		CrosshairStyle shape = this.style.get();
		if (shape == CrosshairStyle.VANILLA) {
			return false;
		}
		Minecraft minecraft = Minecraft.getInstance();
		Window window = minecraft.getWindow();
		int scale = window.getGuiScale();
		float spreadAmount = this.dynamic.get() ? this.spread.update(spreadTarget(minecraft)) * this.dynamicSpread.getFloat() : 0.0F;
		Layout layout = new Layout(
			window.getWidth() / 2,
			window.getHeight() / 2,
			pixels(this.thickness.get(), scale),
			pixels(this.length.get(), scale),
			Math.round((this.gap.getFloat() + spreadAmount) * scale)
		);
		CrosshairBlend shownBlend = this.blend.get();
		Matrix3x2fStack pose = graphics.pose();
		pose.pushMatrix();
		try {
			pose.scale(1.0F / scale);
			GuiMesh mesh = GuiMesh.colored(graphics, shownBlend.pipeline());
			if (shownBlend == CrosshairBlend.NORMAL && this.outline.get()) {
				shapes(mesh, shape, layout, pixels(this.outlineThickness.get(), scale), this.outlineColor.argb());
			}
			shapes(mesh, shape, layout, 0, fillColor(minecraft));
			mesh.submit(graphics);
		} finally {
			pose.popMatrix();
		}
		return true;
	}

	private void shapes(GuiMesh mesh, CrosshairStyle shape, Layout layout, int grow, int argb) {
		int x0 = layout.x0();
		int y0 = layout.y0();
		int x1 = layout.x1();
		int y1 = layout.y1();
		int t = layout.thickness();
		int l = layout.length();
		int g = layout.gap();
		switch (shape) {
			case VANILLA -> {
			}
			case CROSS, T_SHAPE -> {
				box(mesh, x0 - g - l, y0, x0 - g, y1, grow, argb);
				box(mesh, x1 + g, y0, x1 + g + l, y1, grow, argb);
				if (shape == CrosshairStyle.CROSS) {
					box(mesh, x0, y0 - g - l, x1, y0 - g, grow, argb);
				}
				box(mesh, x0, y1 + g, x1, y1 + g + l, grow, argb);
			}
			case SQUARE -> {
				box(mesh, x0 - g - t, y0 - g - t, x1 + g + t, y0 - g, grow, argb);
				box(mesh, x0 - g - t, y1 + g, x1 + g + t, y1 + g + t, grow, argb);
				box(mesh, x0 - g - t, y0 - g, x0 - g, y1 + g, grow, argb);
				box(mesh, x1 + g, y0 - g, x1 + g + t, y1 + g, grow, argb);
			}
			case CIRCLE -> mesh.ring(layout.centerX(), layout.centerY(), g + t, t + grow * 2.0F, argb, FEATHER);
			case X_SHAPE -> {
				for (int dx = -1; dx <= 1; dx += 2) {
					for (int dy = -1; dy <= 1; dy += 2) {
						diagonal(mesh, layout, dx * DIAGONAL, dy * DIAGONAL, g, g + l, grow, argb);
					}
				}
			}
			case ARROW -> {
				diagonal(mesh, layout, -DIAGONAL, DIAGONAL, 0.0F, l, grow, argb);
				diagonal(mesh, layout, DIAGONAL, DIAGONAL, 0.0F, l, grow, argb);
			}
			case DOT -> box(mesh, x0, y0, x1, y1, grow, argb);
		}
		if (shape != CrosshairStyle.DOT && this.dot.get()) {
			box(mesh, x0, y0, x1, y1, grow, argb);
		}
	}

	private static void diagonal(GuiMesh mesh, Layout layout, float dx, float dy, float from, float to, int grow, int argb) {
		float start = Math.max(0.0F, from - grow);
		float end = to + grow;
		mesh.segment(
			layout.centerX() + dx * start,
			layout.centerY() + dy * start,
			layout.centerX() + dx * end,
			layout.centerY() + dy * end,
			layout.thickness() + grow * 2.0F,
			argb
		);
	}

	private static void box(GuiMesh mesh, int x0, int y0, int x1, int y1, int grow, int argb) {
		mesh.rect(x0 - grow, y0 - grow, x1 + grow, y1 + grow, argb);
	}

	private int fillColor(Minecraft minecraft) {
		if (this.highlight.get() && minecraft.crosshairPickEntity instanceof LivingEntity living && living.isAlive()) {
			return this.highlightColor.argb();
		}
		return this.color.argb();
	}

	private static float spreadTarget(Minecraft minecraft) {
		LocalPlayer player = minecraft.player;
		if (player == null) {
			return 0.0F;
		}
		ItemStack using = player.getUseItem();
		if (player.isUsingItem() && using.getItem() instanceof BowItem) {
			return 1.0F - BowItem.getPowerForTime(player.getTicksUsingItem());
		}
		if (player.isUsingItem() && using.getItem() instanceof CrossbowItem) {
			return 1.0F - Mth.clamp((float) player.getTicksUsingItem() / CrossbowItem.getChargeDuration(using, player), 0.0F, 1.0F);
		}
		Vec3 motion = player.getDeltaMovement();
		float movement = (float) Mth.clamp(Math.sqrt(motion.x * motion.x + motion.z * motion.z) / SPRINT_SPEED, 0.0, 1.0) * MOVEMENT_SPREAD;
		float airborne = player.onGround() ? 0.0F : AIRBORNE_SPREAD;
		float cooldown = (1.0F - player.getAttackStrengthScale(0.0F)) * COOLDOWN_SPREAD;
		return movement + airborne + cooldown;
	}

	private static int pixels(double guiPixels, int scale) {
		return Math.max(1, (int) Math.round(guiPixels * scale));
	}

	private boolean isCustom() {
		return !this.style.is(CrosshairStyle.VANILLA);
	}

	private boolean hasArms() {
		return switch (this.style.get()) {
			case CROSS, T_SHAPE, X_SHAPE, ARROW -> true;
			case VANILLA, CIRCLE, SQUARE, DOT -> false;
		};
	}

	private boolean hasGap() {
		return switch (this.style.get()) {
			case CROSS, T_SHAPE, X_SHAPE, CIRCLE, SQUARE -> true;
			case VANILLA, ARROW, DOT -> false;
		};
	}

	private boolean isOutlinable() {
		return isCustom() && this.blend.is(CrosshairBlend.NORMAL);
	}

	private record Layout(int centerPixelX, int centerPixelY, int thickness, int length, int gap) {
		int x0() {
			return this.centerPixelX - this.thickness / 2;
		}

		int y0() {
			return this.centerPixelY - this.thickness / 2;
		}

		int x1() {
			return x0() + this.thickness;
		}

		int y1() {
			return y0() + this.thickness;
		}

		float centerX() {
			return x0() + this.thickness / 2.0F;
		}

		float centerY() {
			return y0() + this.thickness / 2.0F;
		}
	}
}
