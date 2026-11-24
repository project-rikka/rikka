package sh.rikka.client.mods.hud;

import java.util.Locale;
import java.util.Objects;
import net.minecraft.ChatFormatting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.util.ARGB;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.LivingEntity;
import org.joml.Matrix3x2fStack;
import org.jspecify.annotations.Nullable;
import os.rikka.Align;
import os.rikka.Color;
import os.rikka.ColorRole;
import os.rikka.Element;
import os.rikka.FontWeight;
import os.rikka.View;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.hud.style.HudLook;
import sh.rikka.client.mod.HudCanvas;
import sh.rikka.client.mod.OverlayHudMod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ColorSetting;
import sh.rikka.client.setting.ScreenPosition;
import sh.rikka.client.ui.Smoothed;
import sh.rikka.client.ui.style.Colors;

public final class TargetInfoMod extends OverlayHudMod {
	private static final float WIDTH = 176.0F;
	private static final float HEIGHT = 54.0F;
	private static final float PADDING = 7.0F;
	private static final float PORTRAIT = HEIGHT - PADDING * 2.0F;
	private static final float PORTRAIT_RADIUS = 6.0F;
	private static final float GAP = 8.0F;
	private static final float NAME_SIZE = 13.0F;
	private static final float DETAIL_SIZE = 11.0F;
	private static final float DETAIL_ICON = 12.0F;
	private static final float BAR_HEIGHT = 5.0F;
	private static final float BAR_TOP = 22.0F;
	private static final float DETAIL_TOP = 32.0F;
	private static final float APPEAR_MILLIS = 70.0F;
	private static final float HEALTH_MILLIS = 90.0F;
	private static final float TRAIL_MILLIS = 380.0F;
	private static final float HIDDEN_SCALE = 0.92F;
	private static final float EPSILON = 0.001F;
	private static final float MODEL_VISIBILITY = 0.5F;
	private static final int HURT_TINT = 0xFFFF7070;
	private static final Color TRACK = new Color(1.0F, 1.0F, 1.0F, 0.12F);
	private static final Color TRAIL = new Color(1.0F, 1.0F, 1.0F, 0.55F);
	private static final Color ABSORPTION = new Color(1.0F, 0.82F, 0.25F, 1.0F);

	private final BoolSetting background = bool("background", true);
	private final BoolSetting portrait = bool("portrait", true);
	private final BoolSetting armor = bool("armor", true);
	private final BoolSetting distance = bool("distance", true);
	private final BoolSetting dynamicColor = bool("dynamic_color", true);
	private final ColorSetting barColor = color("bar_color", 0xFF4ADE80).opaque().visibleWhen(() -> !this.dynamicColor.get());
	private final Smoothed visibility = new Smoothed(APPEAR_MILLIS, EPSILON, 0.0F);
	private final Smoothed health = new Smoothed(HEALTH_MILLIS, EPSILON, 0.0F);
	private final Smoothed trail = new Smoothed(TRAIL_MILLIS, EPSILON, 0.0F);
	private @Nullable LivingEntity shown;

	public TargetInfoMod(ScreenPosition defaultPosition) {
		super("target_info", defaultPosition, "person_search");
	}

	@Override
	protected void onDisable() {
		this.shown = null;
		this.visibility.snap(0.0F);
	}

	@Override
	public float width() {
		return WIDTH;
	}

	@Override
	public float height() {
		return HEIGHT;
	}

	@Override
	public boolean animating() {
		LivingEntity target = target();
		return target != this.shown && target != null
			|| (target == null) != (this.visibility.value() == 0.0F)
			|| !this.visibility.isSettled()
			|| !this.health.isSettled()
			|| !this.trail.isSettled();
	}

	@Override
	protected void view(View view, HudLook look) {
		LivingEntity target = target();
		if (target != null && target != this.shown) {
			boolean appearing = this.shown == null || this.visibility.value() == 0.0F;
			this.shown = target;
			if (appearing) {
				float fraction = healthFraction(target);
				this.health.snap(fraction);
				this.trail.snap(fraction);
			}
		}
		float shownVisibility = this.visibility.update(target != null ? 1.0F : 0.0F);
		LivingEntity entity = this.shown;
		if (entity == null || shownVisibility == 0.0F) {
			if (shownVisibility == 0.0F) {
				this.shown = null;
			}
			return;
		}
		float fraction = healthFraction(entity);
		float shownHealth = this.health.update(fraction);
		float shownTrail = this.trail.update(fraction);
		view.column(() -> content(view, look, entity, shownHealth, Math.max(shownTrail, shownHealth)))
			.position(0.0F, 0.0F)
			.width(WIDTH)
			.height(HEIGHT)
			.opacity(shownVisibility)
			.scale(Mth.lerp(shownVisibility, HIDDEN_SCALE, 1.0F));
	}

	private void content(View view, HudLook look, LivingEntity entity, float health, float trail) {
		Element panel = view.box(() -> {
		}).position(0.0F, 0.0F).width(WIDTH).height(HEIGHT);
		if (this.background.get()) {
			look.panel(panel);
		}
		float left = textLeft();
		float barWidth = WIDTH - left - PADDING;
		if (this.portrait.get()) {
			view.box(() -> {
			}).position(PADDING, PADDING).width(PORTRAIT).height(PORTRAIT).cornerRadius(PORTRAIT_RADIUS).background(TRACK);
		}
		view.box(() -> look.text(view.text(name(entity)).textSize(NAME_SIZE).textWeight(FontWeight.MEDIUM).textNoWrap()))
			.position(left, PADDING - 1.0F)
			.width(barWidth)
			.clipContent();
		bar(view, left, barWidth, 1.0F, TRACK);
		bar(view, left, barWidth, trail, TRAIL);
		bar(view, left, barWidth, health, healthColor(health));
		float absorption = Math.min(1.0F, entity.getAbsorptionAmount() / Math.max(entity.getMaxHealth(), 1.0F));
		if (absorption > 0.0F) {
			view.box(() -> {
			}).position(left, BAR_TOP + BAR_HEIGHT - 1.5F).width(barWidth * absorption).height(1.5F).background(ABSORPTION);
		}
		view.row(() -> details(view, look, entity)).position(left, DETAIL_TOP).width(barWidth).gap(4.0F).alignItems(Align.CENTER);
	}

	private void details(View view, HudLook look, LivingEntity entity) {
		view.icon("favorite").iconSize(DETAIL_ICON).iconFilled().tint(view.paletteColor(ColorRole.DANGER));
		look.text(view.text(String.format(Locale.ROOT, "%.1f", entity.getHealth() + entity.getAbsorptionAmount())).textSize(DETAIL_SIZE).textNoWrap());
		if (this.armor.get()) {
			view.icon("shield").iconSize(DETAIL_ICON).iconFilled().tint(view.paletteColor(ColorRole.TEXT_MUTED)).marginEdges(0.0F, 0.0F, 0.0F, 4.0F);
			look.text(view.text(Integer.toString(entity.getArmorValue())).textSize(DETAIL_SIZE).textNoWrap());
		}
		LocalPlayer player = Minecraft.getInstance().player;
		if (this.distance.get() && player != null && entity != player) {
			view.spacer();
			look.mutedText(view.text(String.format(Locale.ROOT, "%.1f m", entity.distanceTo(player))).textSize(DETAIL_SIZE).textNoWrap());
		}
	}

	private static void bar(View view, float left, float width, float fraction, Color color) {
		if (fraction <= 0.0F) {
			return;
		}
		view.box(() -> {
		}).position(left, BAR_TOP).width(width * Math.min(fraction, 1.0F)).height(BAR_HEIGHT).cornerRadius(BAR_HEIGHT / 2.0F).background(color);
	}

	@Override
	protected void extract(HudCanvas canvas) {
		LivingEntity entity = this.shown;
		float shownVisibility = this.visibility.value();
		if (entity == null || shownVisibility == 0.0F || !this.portrait.get()) {
			return;
		}
		Matrix3x2fStack pose = canvas.graphics().pose();
		pose.pushMatrix();
		try {
			float scale = Mth.lerp(shownVisibility, HIDDEN_SCALE, 1.0F);
			pose.scale(scale);
			float inset = PORTRAIT * 0.1F;
			if (entity instanceof AbstractClientPlayer player) {
				int tint = entity.hurtTime > 0 ? HURT_TINT : 0xFFFFFFFF;
				canvas.face(player.getSkin(), PADDING + inset, PADDING + inset, PORTRAIT - inset * 2.0F, ARGB.multiplyAlpha(tint, shownVisibility));
			} else if (shownVisibility >= MODEL_VISIBILITY) {
				canvas.entity(entity, PADDING, PADDING, PORTRAIT, PORTRAIT);
			}
		} finally {
			pose.popMatrix();
		}
	}

	private float textLeft() {
		return this.portrait.get() ? PADDING + PORTRAIT + GAP : PADDING;
	}

	private Color healthColor(float fraction) {
		if (!this.dynamicColor.get()) {
			return Colors.of(this.barColor.argb());
		}
		float hue = Mth.clamp(fraction, 0.0F, 1.0F) / 3.0F;
		return Colors.of(Mth.hsvToArgb(hue, 0.7F, 0.95F, 255));
	}

	private static @Nullable LivingEntity target() {
		if (editing()) {
			return Minecraft.getInstance().player;
		}
		return RikkaClient.targets().target();
	}

	private static float healthFraction(LivingEntity entity) {
		return Mth.clamp(entity.getHealth() / Math.max(entity.getMaxHealth(), 1.0F), 0.0F, 1.0F);
	}

	private static String name(LivingEntity entity) {
		return Objects.requireNonNullElse(ChatFormatting.stripFormatting(entity.getDisplayName().getString()), "");
	}
}
