package sh.rikka.client.mods.rearview;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.DefaultVertexFormat;
import com.mojang.renderpearl.api.pipeline.BlendFunction;
import com.mojang.renderpearl.api.pipeline.ColorTargetState;
import com.mojang.renderpearl.api.pipeline.PrimitiveTopology;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import com.mojang.renderpearl.api.textures.FilterMode;
import com.mojang.renderpearl.api.textures.GpuTextureView;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.BindGroupLayouts;
import net.minecraft.resources.Identifier;
import net.minecraft.util.ARGB;
import net.minecraft.util.Mth;
import org.jspecify.annotations.Nullable;
import os.rikka.Element;
import os.rikka.View;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.ResourceReloadEvent;
import sh.rikka.client.hud.style.HudLook;
import sh.rikka.client.mod.HudCanvas;
import sh.rikka.client.mod.OverlayHudMod;
import sh.rikka.client.render.GuiMesh;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.IntSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class RearviewMod extends OverlayHudMod {
	private static final float WIDTH = 256.0F;
	private static final float HEIGHT = 144.0F;
	private static final float INSET = 2.0F;
	private static final float CORNER_RADIUS = 12.0F;
	private static final int CORNER_SEGMENTS = 8;
	private static final RenderPipeline PIPELINE = RenderPipeline.builder()
		.withLocation(Identifier.fromNamespaceAndPath(RikkaClient.MOD_ID, "pipeline/rearview"))
		.withBindGroupLayout(BindGroupLayouts.GLOBALS)
		.withBindGroupLayout(BindGroupLayouts.PROJECTION)
		.withBindGroupLayout(BindGroupLayouts.DYNAMIC_TRANSFORMS)
		.withVertexShader("core/position_tex_color")
		.withFragmentShader(Identifier.fromNamespaceAndPath(RikkaClient.MOD_ID, "core/rearview"))
		.withBindGroupLayout(BindGroupLayouts.SAMPLER0)
		.withColorTargetState(new ColorTargetState(BlendFunction.TRANSLUCENT))
		.withVertexBinding(0, DefaultVertexFormat.POSITION_TEX_COLOR)
		.withPrimitiveTopology(PrimitiveTopology.QUADS)
		.build();
	private static volatile @Nullable RearviewMod active;

	private final IntSetting fov = integer("fov", 70, 30, 110);
	private final IntSetting frameRate = integer("frame_rate", 60, 10, 240).step(5);
	private final IntSetting renderDistance = integer("render_distance", 8, 2, 32);
	private final DoubleSetting resolution = number("resolution", 1.0, 0.5, 2.0).step(0.25);
	private final BoolSetting followPitch = bool("follow_pitch", false);
	private final BoolSetting mirror = bool("mirror", false);
	private final DoubleSetting opacity = number("opacity", 1.0, 0.2, 1.0).step(0.05);
	private final BoolSetting background = bool("background", true);
	private @Nullable RearviewRenderer renderer;

	public RearviewMod(ScreenPosition defaultPosition) {
		super("rearview", defaultPosition, "camera_rear");
	}

	static @Nullable RearviewMod active() {
		return active;
	}

	@Override
	protected void onEnable() {
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
		RearviewRenderer released = this.renderer;
		if (released != null) {
			this.renderer = null;
			released.detach();
			Minecraft.getInstance().schedule(released::close);
		}
	}

	@EventHandler
	private void onReload(ResourceReloadEvent event) {
		if (this.renderer != null) {
			this.renderer.reloadResources();
		}
	}

	void render(RearviewRenderer.MainView main) {
		if (this.renderer != null) {
			this.renderer.render(main);
		}
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
	protected void view(View view, HudLook look) {
		Element frame = view.box(() -> {
		}).position(0.0F, 0.0F).width(WIDTH).height(HEIGHT);
		if (this.background.get()) {
			look.panel(frame, CORNER_RADIUS);
		}
	}

	@Override
	protected void extract(HudCanvas canvas) {
		Minecraft minecraft = Minecraft.getInstance();
		if (minecraft.level == null || minecraft.getCameraEntity() == null) {
			return;
		}
		float inset = this.background.get() ? INSET : 0.0F;
		float imageWidth = WIDTH - 2.0F * inset;
		float imageHeight = HEIGHT - 2.0F * inset;
		float pixel = canvas.pixel();
		int maxSize = RenderSystem.getDevice().getDeviceInfo().limits().maxTextureSize();
		double samples = this.resolution.get() / pixel;
		RearviewRenderer.Request request = new RearviewRenderer.Request(
			Mth.clamp((int) Math.round(imageWidth * samples), 1, maxSize),
			Mth.clamp((int) Math.round(imageHeight * samples), 1, maxSize),
			this.fov.get(),
			this.renderDistance.get(),
			this.followPitch.get(),
			this.frameRate.get()
		);
		GpuTextureView image = renderer().request(request);
		float[] outline = GuiMesh.roundedRectangle(
			WIDTH / 2.0F,
			HEIGHT / 2.0F,
			imageWidth / 2.0F,
			imageHeight / 2.0F,
			CORNER_RADIUS - inset,
			CORNER_SEGMENTS
		);
		int tint = ARGB.white(this.opacity.getFloat());
		GuiMesh.textured(canvas.graphics(), PIPELINE, image, RenderSystem.getSamplerCache().getClampToEdge(FilterMode.LINEAR))
			.convex(outline, tint, pixel, new ImageUv(inset, inset, imageWidth, imageHeight, this.mirror.get()))
			.submit(canvas.graphics());
	}

	private RearviewRenderer renderer() {
		RearviewRenderer renderer = this.renderer;
		if (renderer == null) {
			renderer = new RearviewRenderer();
			this.renderer = renderer;
		}
		return renderer;
	}

	private record ImageUv(float left, float top, float width, float height, boolean mirrored) implements GuiMesh.UvMap {
		@Override
		public float u(float x, float y) {
			float u = (x - this.left) / this.width;
			return this.mirrored ? 1.0F - u : u;
		}

		@Override
		public float v(float x, float y) {
			return 1.0F - (y - this.top) / this.height;
		}
	}
}
