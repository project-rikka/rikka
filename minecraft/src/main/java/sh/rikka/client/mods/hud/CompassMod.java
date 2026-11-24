package sh.rikka.client.mods.hud;

import java.util.EnumMap;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import net.minecraft.client.Camera;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.resources.language.I18n;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.util.Mth;
import net.minecraft.world.phys.Vec3;
import org.jspecify.annotations.Nullable;
import os.rikka.Element;
import os.rikka.FontWeight;
import os.rikka.Justification;
import os.rikka.View;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.ResourceReloadEvent;
import sh.rikka.client.hud.style.HudLook;
import sh.rikka.client.mod.HudMod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.IntSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class CompassMod extends HudMod {
	private static final float HEIGHT = 40.0F;
	private static final int STEP_DEGREES = 15;
	private static final int INTERCARDINAL_DEGREES = 45;
	private static final int CARDINAL_DEGREES = 90;
	private static final int FULL_TURN = 360;
	private static final float HALF_TURN = 180.0F;
	private static final float EDGE_FADE = 0.35F;
	private static final float SLOT_WIDTH = 48.0F;
	private static final float READOUT_TOP = 2.0F;
	private static final float READOUT_SIZE = 10.0F;
	private static final float POINTER_TOP = -1.0F;
	private static final float POINTER_SIZE = 16.0F;
	private static final float TICK_TOP = 15.0F;
	private static final float TICK_WIDTH = 1.0F;
	private static final float CARDINAL_TICK = 8.0F;
	private static final float INTERCARDINAL_TICK = 6.0F;
	private static final float MINOR_TICK = 4.0F;
	private static final float NEEDLE_WIDTH = 2.0F;
	private static final float NEEDLE_HEIGHT = 9.0F;
	private static final float LABEL_TOP = 24.0F;
	private static final float DEGREES_TOP = 25.0F;
	private static final float CARDINAL_SIZE = 12.0F;
	private static final float INTERCARDINAL_SIZE = 10.0F;
	private static final float DEGREES_SIZE = 8.0F;
	private static final float MARKER_SIZE = 12.0F;
	private static final float MARKER_TOP = TICK_TOP - 2.0F;
	private static final float MARKER_INSET = MARKER_SIZE;
	private static final float OFFSCREEN_MARKER = 0.55F;

	private final IntSetting width = integer("width", 240, 120, 480).step(10);
	private final IntSetting span = integer("span", 180, 90, 360).step(STEP_DEGREES);
	private final BoolSetting heading = bool("heading", true);
	private final BoolSetting degrees = bool("degrees", true);
	private final BoolSetting markers = bool("markers", true);
	private final BoolSetting background = bool("background", true);
	private @Nullable Map<Compass, String> labels;
	private float shownYaw;
	private Vec3 shownPosition = Vec3.ZERO;

	public CompassMod(ScreenPosition defaultPosition) {
		super("compass", defaultPosition, "explore");
	}

	@Override
	public float width() {
		return this.width.get();
	}

	@Override
	public float height() {
		return HEIGHT;
	}

	@Override
	public boolean animating() {
		if (Minecraft.getInstance().player == null) {
			return false;
		}
		Camera camera = Minecraft.getInstance().gameRenderer.mainCamera();
		return camera.yRot() != this.shownYaw || this.markers.get() && !camera.position().equals(this.shownPosition);
	}

	@Override
	protected void onEnable() {
		this.labels = null;
	}

	@EventHandler
	private void onResourceReload(ResourceReloadEvent event) {
		this.labels = null;
	}

	@Override
	protected void view(View view, HudLook look) {
		Camera camera = Minecraft.getInstance().gameRenderer.mainCamera();
		this.shownYaw = camera.yRot();
		this.shownPosition = camera.position();
		float facing = Minecraft.getInstance().player == null ? 0.0F : camera.yRot() + HALF_TURN;
		float heading = Mth.positiveModulo(facing, FULL_TURN);
		Element panel = view.box(() -> strip(view, look, camera, heading))
			.fillWidth()
			.fillHeight()
			.clipContent();
		if (this.background.get()) {
			look.panel(panel);
		}
	}

	private void strip(View view, HudLook look, Camera camera, float heading) {
		float boxWidth = width();
		float center = boxWidth / 2.0F;
		float pixelsPerDegree = boxWidth / this.span.get();
		float halfSpan = this.span.get() / 2.0F;
		Map<Compass, String> points = labels();
		int first = Mth.floor((heading - halfSpan) / STEP_DEGREES) * STEP_DEGREES;
		for (int angle = first; angle <= heading + halfSpan; angle += STEP_DEGREES) {
			float x = center + (angle - heading) * pixelsPerDegree;
			float opacity = edgeOpacity(x, center);
			if (opacity <= 0.0F) {
				continue;
			}
			int bearing = Math.floorMod(angle, FULL_TURN);
			if (bearing % CARDINAL_DEGREES == 0) {
				tick(view, look, x, CARDINAL_TICK, opacity, false);
				String point = points.get(Compass.fromYaw(bearing - HALF_TURN));
				label(view, x, LABEL_TOP, opacity, () -> look.text(view.text(point)
					.textSize(CARDINAL_SIZE)
					.textWeight(FontWeight.SEMIBOLD)
					.textNoWrap()));
			} else if (bearing % INTERCARDINAL_DEGREES == 0) {
				tick(view, look, x, INTERCARDINAL_TICK, opacity, false);
				String point = points.get(Compass.fromYaw(bearing - HALF_TURN));
				label(view, x, LABEL_TOP, opacity, () -> look.text(view.text(point)
					.textSize(INTERCARDINAL_SIZE)
					.textNoWrap()));
			} else {
				tick(view, look, x, MINOR_TICK, opacity, true);
				if (this.degrees.get()) {
					String degreeLabel = Integer.toString(bearing);
					label(view, x, DEGREES_TOP, opacity, () -> look.mutedText(view.text(degreeLabel)
						.textSize(DEGREES_SIZE)
						.textNoWrap()));
				}
			}
		}
		if (this.markers.get()) {
			markers(view, look, camera, heading, center, pixelsPerDegree);
		}
		look.mark(view.box(() -> {
			}))
			.position(center - NEEDLE_WIDTH / 2.0F, TICK_TOP)
			.width(NEEDLE_WIDTH)
			.height(NEEDLE_HEIGHT)
			.cornerRadius(NEEDLE_WIDTH / 2.0F);
		if (this.heading.get()) {
			String readout = String.format(Locale.ROOT, "%d°", Math.round(heading) % FULL_TURN);
			label(view, center, READOUT_TOP, 1.0F, () -> look.text(view.text(readout)
				.textSize(READOUT_SIZE)
				.textWeight(FontWeight.MEDIUM)
				.textNoWrap()));
		} else {
			label(view, center, POINTER_TOP, 1.0F, () -> look.text(view.icon("arrow_drop_down").iconSize(POINTER_SIZE)));
		}
	}

	private void markers(View view, HudLook look, Camera camera, float heading, float center, float pixelsPerDegree) {
		Minecraft minecraft = Minecraft.getInstance();
		ClientLevel level = minecraft.level;
		LocalPlayer player = minecraft.player;
		if (level == null || player == null) {
			return;
		}
		marker(view, look, camera, level, Optional.of(level.getRespawnData().globalPos()), "home", heading, center, pixelsPerDegree);
		marker(view, look, camera, level, player.getLastDeathLocation(), "skull", heading, center, pixelsPerDegree);
	}

	private void marker(
		View view,
		HudLook look,
		Camera camera,
		ClientLevel level,
		Optional<GlobalPos> target,
		String icon,
		float heading,
		float center,
		float pixelsPerDegree
	) {
		if (target.isEmpty() || !target.get().dimension().equals(level.dimension())) {
			return;
		}
		BlockPos block = target.get().pos();
		Vec3 eye = camera.position();
		double east = block.getX() + 0.5 - eye.x;
		double south = block.getZ() + 0.5 - eye.z;
		if (east * east + south * south < 1.0) {
			return;
		}
		float bearing = (float) Math.toDegrees(Math.atan2(east, -south));
		float x = center + Mth.wrapDegrees(bearing - heading) * pixelsPerDegree;
		float clamped = Math.clamp(x, MARKER_INSET, width() - MARKER_INSET);
		float opacity = clamped == x ? 1.0F : OFFSCREEN_MARKER;
		label(view, clamped, MARKER_TOP, opacity, () -> look.text(view.icon(icon).iconSize(MARKER_SIZE).iconFilled()));
	}

	private static void tick(View view, HudLook look, float x, float length, float opacity, boolean muted) {
		Element tick = view.box(() -> {
			})
			.position(x - TICK_WIDTH / 2.0F, TICK_TOP)
			.width(TICK_WIDTH)
			.height(length)
			.opacity(opacity);
		if (muted) {
			look.mutedMark(tick);
		} else {
			look.mark(tick);
		}
	}

	private static void label(View view, float x, float top, float opacity, Runnable content) {
		view.row(content)
			.position(x - SLOT_WIDTH / 2.0F, top)
			.width(SLOT_WIDTH)
			.justify(Justification.CENTER)
			.opacity(opacity);
	}

	private float edgeOpacity(float x, float center) {
		float distance = Math.abs(x - center);
		float fadeWidth = center * EDGE_FADE;
		float visible = Math.clamp((center - distance) / fadeWidth, 0.0F, 1.0F);
		return visible * visible * (3.0F - 2.0F * visible);
	}

	private Map<Compass, String> labels() {
		Map<Compass, String> current = this.labels;
		if (current == null) {
			current = new EnumMap<>(Compass.class);
			for (Compass point : Compass.values()) {
				current.put(point, I18n.get(translationKey() + "." + point.translationSuffix()));
			}
			this.labels = current;
		}
		return current;
	}
}
