package sh.rikka.client.mods.minimap;

import com.mojang.blaze3d.platform.InputConstants;
import java.util.Locale;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.resources.language.I18n;
import net.minecraft.core.BlockPos;
import net.minecraft.core.SectionPos;
import net.minecraft.util.ARGB;
import net.minecraft.util.Mth;
import net.minecraft.world.level.LightLayer;
import os.rikka.Align;
import os.rikka.Element;
import os.rikka.Justification;
import os.rikka.View;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.BlockUpdateEvent;
import sh.rikka.client.events.ChunkLoadEvent;
import sh.rikka.client.events.InputAction;
import sh.rikka.client.events.KeyInputEvent;
import sh.rikka.client.events.ResourceReloadEvent;
import sh.rikka.client.events.TickEvent;
import sh.rikka.client.hud.style.HudLook;
import sh.rikka.client.mod.HudCanvas;
import sh.rikka.client.mod.OverlayHudMod;
import sh.rikka.client.render.GuiMesh;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.IntSetting;
import sh.rikka.client.setting.KeybindSetting;
import sh.rikka.client.setting.ScreenPosition;
import sh.rikka.client.ui.Smoothed;

public final class MinimapMod extends OverlayHudMod {
	private static final double MIN_ZOOM = 0.5;
	private static final double MAX_ZOOM = 8.0;
	private static final double ZOOM_FACTOR = 2.0;
	private static final float ZOOM_MILLIS = 80.0F;
	private static final float ZOOM_EPSILON = 0.001F;
	private static final float INSET = 2.0F;
	private static final float COORDINATES_GAP = 4.0F;
	private static final float COORDINATES_HEIGHT = 18.0F;
	private static final float COORDINATES_SIZE = 11.0F;
	private static final float COORDINATES_RADIUS = 7.0F;
	private static final int CAVE_HEADROOM = 2;
	private static final int CAVE_HYSTERESIS = 4;
	private static final String[] CARDINALS = {"north", "east", "south", "west"};

	private final IntSetting size = integer("size", 160, 80, 320);
	private final EnumSetting<MapShape> shape = choice("shape", MapShape.WIDE);
	private final BoolSetting rotate = bool("rotate", true);
	private final DoubleSetting zoom = number("zoom", 2.0, MIN_ZOOM, MAX_ZOOM).step(0.25);
	private final KeybindSetting zoomIn = keybind("zoom_in", InputConstants.KEY_RBRACKET);
	private final KeybindSetting zoomOut = keybind("zoom_out", InputConstants.KEY_LBRACKET);
	private final EnumSetting<MapLayer> layer = choice("layer", MapLayer.AUTO);
	private final EnumSetting<TerrainColors> colors = choice("colors", TerrainColors.BLOCKS);
	private final DoubleSetting opacity = number("opacity", 1.0, 0.2, 1.0).step(0.05);
	private final BoolSetting players = bool("players", true);
	private final BoolSetting hostiles = bool("hostiles", true);
	private final BoolSetting animals = bool("animals", false);
	private final BoolSetting cursor = bool("cursor", true);
	private final BoolSetting cardinals = bool("cardinals", true);
	private final BoolSetting coordinates = bool("coordinates", true);
	private final BoolSetting background = bool("background", true);
	private final TerrainCache terrain = new TerrainCache();
	private final Smoothed shownZoom = new Smoothed(ZOOM_MILLIS, ZOOM_EPSILON, 1.0F);
	private int caveCeiling = TerrainScanner.SURFACE;

	public MinimapMod(ScreenPosition defaultPosition) {
		super("minimap", defaultPosition, "explore");
	}

	@Override
	protected void onEnable() {
		this.shownZoom.snap((float) log2(this.zoom.get()));
		this.caveCeiling = TerrainScanner.SURFACE;
	}

	@Override
	protected void onDisable() {
		this.terrain.close();
	}

	@EventHandler
	private void onChunkLoad(ChunkLoadEvent event) {
		this.terrain.markChunk(event.chunkX(), event.chunkZ());
	}

	@EventHandler
	private void onBlockUpdate(BlockUpdateEvent event) {
		this.terrain.markBlock(event.pos());
	}

	@EventHandler
	private void onReload(ResourceReloadEvent event) {
		this.terrain.reloadColors();
	}

	@EventHandler
	private void onTick(TickEvent.Post event) {
		if (Minecraft.getInstance().level == null) {
			this.terrain.detach();
		}
	}

	@EventHandler
	private void onKey(KeyInputEvent event) {
		if (event.action() == InputAction.RELEASE || Minecraft.getInstance().gui.screen() != null) {
			return;
		}
		InputConstants.Key key = event.key();
		if (this.zoomIn.matches(key)) {
			this.zoom.set(Math.min(MAX_ZOOM, this.zoom.get() * ZOOM_FACTOR));
		} else if (this.zoomOut.matches(key)) {
			this.zoom.set(Math.max(MIN_ZOOM, this.zoom.get() / ZOOM_FACTOR));
		}
	}

	@Override
	public float width() {
		return this.size.get();
	}

	@Override
	public float height() {
		return mapHeight() + (this.coordinates.get() ? COORDINATES_GAP + COORDINATES_HEIGHT : 0.0F);
	}

	@Override
	protected void view(View view, HudLook look) {
		float side = this.size.get();
		float mapHeight = mapHeight();
		Element frame = view.box(() -> {
		}).position(0.0F, 0.0F).width(side).height(mapHeight);
		if (this.background.get()) {
			look.panel(frame, this.shape.get().cornerRadius(side, mapHeight));
		} else {
			frame.cornerRadius(this.shape.get().cornerRadius(side, mapHeight));
		}
		LocalPlayer player = Minecraft.getInstance().player;
		if (this.coordinates.get() && player != null) {
			BlockPos pos = player.blockPosition();
			String text = String.format(Locale.ROOT, "%d, %d, %d", pos.getX(), pos.getY(), pos.getZ());
			view.row(() -> {
				Element pill = view.box(() -> view.row(() -> look.text(view.text(text).textSize(COORDINATES_SIZE).textNoWrap())).fillHeight().alignItems(Align.CENTER))
					.height(COORDINATES_HEIGHT)
					.shrink(0.0F)
					.paddingEdges(0.0F, COORDINATES_RADIUS, 0.0F, COORDINATES_RADIUS);
				if (this.background.get()) {
					look.panel(pill, Math.min(look.radius(), COORDINATES_HEIGHT / 2.0F));
				}
			}).position(0.0F, mapHeight + COORDINATES_GAP).width(side).justify(Justification.CENTER);
		}
	}

	@Override
	protected void extract(HudCanvas canvas) {
		Minecraft minecraft = Minecraft.getInstance();
		LocalPlayer player = minecraft.player;
		ClientLevel level = minecraft.level;
		if (player == null || level == null) {
			return;
		}
		float partialTick = canvas.partialTick();
		float side = this.size.get();
		float mapHeight = mapHeight();
		float halfWidth = side / 2.0F - INSET;
		float halfHeight = mapHeight / 2.0F - INSET;
		MapShape shownShape = this.shape.get();
		float zoomLevel = (float) Math.pow(2.0, this.shownZoom.update((float) log2(this.zoom.get())));
		double playerX = Mth.lerp(partialTick, player.xo, player.getX());
		double playerZ = Mth.lerp(partialTick, player.zo, player.getZ());
		float yaw = player.getViewYRot(partialTick) * Mth.DEG_TO_RAD;
		float rotation = this.rotate.get() ? yaw + Mth.PI : 0.0F;
		MapProjection projection = new MapProjection(
			side / 2.0F,
			mapHeight / 2.0F,
			halfWidth,
			halfHeight,
			shownShape,
			zoomLevel,
			rotation,
			playerX,
			playerZ
		);

		float reach = shownShape.reach(halfWidth, halfHeight) / zoomLevel;
		int radius = Math.min(TerrainAtlas.SLOTS / 2 - 1, Mth.ceil(reach / TerrainScanner.CHUNK) + 1);
		int ceiling = ceiling(level, player);
		this.terrain.refresh(
			level,
			SectionPos.blockToSectionCoord(playerX),
			SectionPos.blockToSectionCoord(playerZ),
			radius,
			ceiling,
			this.colors.get()
		);

		TerrainAtlas atlas = this.terrain.atlas();
		float feather = canvas.pixel();
		int tint = ARGB.white(this.opacity.getFloat());
		GuiMesh.textured(canvas.graphics(), atlas.view(), atlas.sampler())
			.convex(shownShape.outline(projection.centerX(), projection.centerY(), halfWidth, halfHeight, feather), tint, feather, projection)
			.submit(canvas.graphics());
		MapMarkers.entities(canvas, projection, level, player, new MapMarkers.Selection(this.players.get(), this.hostiles.get(), this.animals.get()));
		if (this.cursor.get()) {
			MapMarkers.arrow(canvas, projection, yaw + Mth.PI - rotation);
		}
		if (this.cardinals.get()) {
			String[] labels = new String[CARDINALS.length];
			for (int i = 0; i < CARDINALS.length; i++) {
				labels[i] = I18n.get(translationKey() + "." + CARDINALS[i]);
			}
			MapMarkers.cardinals(canvas, projection, labels);
		}
	}

	private float mapHeight() {
		return Math.round(this.size.get() * this.shape.get().aspect());
	}

	private int ceiling(ClientLevel level, LocalPlayer player) {
		boolean caves = switch (this.layer.get()) {
			case SURFACE -> false;
			case CAVES -> true;
			case AUTO -> level.dimensionType().hasCeiling() || level.getBrightness(LightLayer.SKY, BlockPos.containing(player.getEyePosition())) == 0;
		};
		if (!caves) {
			this.caveCeiling = TerrainScanner.SURFACE;
			return TerrainScanner.SURFACE;
		}
		int wanted = player.getBlockY() + CAVE_HEADROOM;
		if (this.caveCeiling == TerrainScanner.SURFACE || wanted > this.caveCeiling || wanted < this.caveCeiling - CAVE_HYSTERESIS) {
			this.caveCeiling = wanted;
		}
		return this.caveCeiling;
	}

	private static double log2(double value) {
		return Math.log(value) / Math.log(2.0);
	}
}
