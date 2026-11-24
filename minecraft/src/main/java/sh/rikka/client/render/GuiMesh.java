package sh.rikka.client.render;

import com.mojang.blaze3d.vertex.VertexConsumer;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import com.mojang.renderpearl.api.textures.GpuSampler;
import com.mojang.renderpearl.api.textures.GpuTextureView;
import it.unimi.dsi.fastutil.floats.FloatArrayList;
import it.unimi.dsi.fastutil.ints.IntArrayList;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.navigation.ScreenRectangle;
import net.minecraft.client.gui.render.TextureSetup;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.client.renderer.state.gui.GuiElementRenderState;
import net.minecraft.util.Mth;
import org.joml.Matrix3x2f;
import org.joml.Vector2f;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mixin.GuiGraphicsExtractorAccessor;

public final class GuiMesh implements GuiElementRenderState {
	private static final int MIN_SEGMENTS = 24;
	private static final int MAX_SEGMENTS = 256;
	private static final float SEGMENT_LENGTH_IN_FEATHERS = 3.0F;
	private static final int CORNERS = 4;
	private static final int[] FORWARD = {0, 1, 2, 3};
	private static final int[] REVERSED = {0, 3, 2, 1};

	private final RenderPipeline pipeline;
	private final TextureSetup textureSetup;
	private final boolean textured;
	private final Matrix3x2f pose;
	private final boolean mirrored;
	private final FloatArrayList positions = new FloatArrayList();
	private final FloatArrayList uvs = new FloatArrayList();
	private final IntArrayList colors = new IntArrayList();
	private final float[] cornerX = new float[CORNERS];
	private final float[] cornerY = new float[CORNERS];
	private final float[] cornerU = new float[CORNERS];
	private final float[] cornerV = new float[CORNERS];
	private final int[] cornerColor = new int[CORNERS];
	private int pendingCorners;
	private @Nullable ScreenRectangle bounds;

	private GuiMesh(RenderPipeline pipeline, TextureSetup textureSetup, boolean textured, Matrix3x2f pose) {
		this.pipeline = pipeline;
		this.textureSetup = textureSetup;
		this.textured = textured;
		this.pose = pose;
		this.mirrored = pose.determinant() < 0.0F;
	}

	public static GuiMesh colored(GuiGraphicsExtractor graphics) {
		return colored(graphics, RenderPipelines.GUI);
	}

	public static GuiMesh colored(GuiGraphicsExtractor graphics, RenderPipeline pipeline) {
		return new GuiMesh(pipeline, TextureSetup.noTexture(), false, new Matrix3x2f(graphics.pose()));
	}

	public static GuiMesh textured(GuiGraphicsExtractor graphics, GpuTextureView texture, GpuSampler sampler) {
		return new GuiMesh(RenderPipelines.GUI_TEXTURED, TextureSetup.singleTexture(texture, sampler), true, new Matrix3x2f(graphics.pose()));
	}

	public GuiMesh rect(float x0, float y0, float x1, float y1, int argb) {
		corner(x0, y0, argb);
		corner(x0, y1, argb);
		corner(x1, y1, argb);
		corner(x1, y0, argb);
		return this;
	}

	public GuiMesh triangle(float x0, float y0, float x1, float y1, float x2, float y2, int argb) {
		corner(x0, y0, argb);
		corner(x1, y1, argb);
		corner(x2, y2, argb);
		corner(x2, y2, argb);
		return this;
	}

	public GuiMesh segment(float x0, float y0, float x1, float y1, float width, int argb) {
		float length = Mth.sqrt(Mth.square(x1 - x0) + Mth.square(y1 - y0));
		if (length <= 0.0F) {
			return this;
		}
		float nx = -(y1 - y0) / length * width / 2.0F;
		float ny = (x1 - x0) / length * width / 2.0F;
		corner(x0 + nx, y0 + ny, argb);
		corner(x0 - nx, y0 - ny, argb);
		corner(x1 - nx, y1 - ny, argb);
		corner(x1 + nx, y1 + ny, argb);
		return this;
	}

	public GuiMesh ring(float centerX, float centerY, float radius, float width, int argb, float feather) {
		float inner = Math.max(0.0F, radius - width / 2.0F);
		float outer = radius + width / 2.0F;
		int transparent = transparent(argb);
		int segments = segments(outer, feather);
		float[] radii = {inner - feather / 2.0F, inner + feather / 2.0F, outer - feather / 2.0F, outer + feather / 2.0F};
		int[] tints = {transparent, argb, argb, transparent};
		for (int band = 0; band < radii.length - 1; band++) {
			float from = Math.max(0.0F, radii[band]);
			float to = Math.max(from, radii[band + 1]);
			for (int i = 0; i < segments; i++) {
				float a0 = Mth.TWO_PI * i / segments;
				float a1 = Mth.TWO_PI * (i + 1) / segments;
				float c0 = Mth.cos(a0);
				float s0 = Mth.sin(a0);
				float c1 = Mth.cos(a1);
				float s1 = Mth.sin(a1);
				corner(centerX + c0 * from, centerY + s0 * from, tints[band]);
				corner(centerX + c0 * to, centerY + s0 * to, tints[band + 1]);
				corner(centerX + c1 * to, centerY + s1 * to, tints[band + 1]);
				corner(centerX + c1 * from, centerY + s1 * from, tints[band]);
			}
		}
		return this;
	}

	public GuiMesh disc(float centerX, float centerY, float radius, int argb, float feather) {
		int segments = segments(radius, feather);
		float inner = radius - feather / 2.0F;
		float outer = radius + feather / 2.0F;
		int transparent = transparent(argb);
		for (int i = 0; i < segments; i++) {
			float a0 = Mth.TWO_PI * i / segments;
			float a1 = Mth.TWO_PI * (i + 1) / segments;
			float c0 = Mth.cos(a0);
			float s0 = Mth.sin(a0);
			float c1 = Mth.cos(a1);
			float s1 = Mth.sin(a1);
			corner(centerX, centerY, argb);
			corner(centerX + c0 * inner, centerY + s0 * inner, argb);
			corner(centerX + c1 * inner, centerY + s1 * inner, argb);
			corner(centerX + c1 * inner, centerY + s1 * inner, argb);
			if (feather > 0.0F) {
				corner(centerX + c0 * inner, centerY + s0 * inner, argb);
				corner(centerX + c0 * outer, centerY + s0 * outer, transparent);
				corner(centerX + c1 * outer, centerY + s1 * outer, transparent);
				corner(centerX + c1 * inner, centerY + s1 * inner, argb);
			}
		}
		return this;
	}

	public GuiMesh convex(float[] outline, int argb, float feather, @Nullable UvMap uv) {
		int count = outline.length / 2;
		if (count < 3) {
			return this;
		}
		float centerX = 0.0F;
		float centerY = 0.0F;
		for (int i = 0; i < count; i++) {
			centerX += outline[i * 2];
			centerY += outline[i * 2 + 1];
		}
		centerX /= count;
		centerY /= count;
		float[] normals = normals(outline, centerX, centerY);
		float half = feather / 2.0F;
		int transparent = transparent(argb);
		for (int i = 0; i < count; i++) {
			int j = (i + 1) % count;
			float ix = outline[i * 2] - normals[i * 2] * half;
			float iy = outline[i * 2 + 1] - normals[i * 2 + 1] * half;
			float jx = outline[j * 2] - normals[j * 2] * half;
			float jy = outline[j * 2 + 1] - normals[j * 2 + 1] * half;
			mapped(centerX, centerY, argb, uv);
			mapped(ix, iy, argb, uv);
			mapped(jx, jy, argb, uv);
			mapped(jx, jy, argb, uv);
			if (feather > 0.0F) {
				mapped(ix, iy, argb, uv);
				mapped(outline[i * 2] + normals[i * 2] * half, outline[i * 2 + 1] + normals[i * 2 + 1] * half, transparent, uv);
				mapped(outline[j * 2] + normals[j * 2] * half, outline[j * 2 + 1] + normals[j * 2 + 1] * half, transparent, uv);
				mapped(jx, jy, argb, uv);
			}
		}
		return this;
	}

	public boolean isEmpty() {
		return this.colors.isEmpty();
	}

	public void submit(GuiGraphicsExtractor graphics) {
		if (this.pendingCorners != 0) {
			throw new IllegalStateException("mesh has an unfinished quad of " + this.pendingCorners + " corners");
		}
		if (isEmpty()) {
			return;
		}
		this.bounds = computeBounds();
		((GuiGraphicsExtractorAccessor) graphics).rikka$guiRenderState().addGuiElement(this);
	}

	public static float[] circle(float centerX, float centerY, float radius, int segments) {
		float[] outline = new float[segments * 2];
		for (int i = 0; i < segments; i++) {
			float angle = Mth.TWO_PI * i / segments;
			outline[i * 2] = centerX + Mth.cos(angle) * radius;
			outline[i * 2 + 1] = centerY + Mth.sin(angle) * radius;
		}
		return outline;
	}

	public static int segments(float radius, float feather) {
		float step = Math.max(feather * SEGMENT_LENGTH_IN_FEATHERS, 1.0E-3F);
		return Mth.clamp(Mth.ceil(Mth.TWO_PI * radius / step), MIN_SEGMENTS, MAX_SEGMENTS);
	}

	@Override
	public void buildVertices(VertexConsumer consumer) {
		for (int i = 0; i < this.colors.size(); i++) {
			VertexConsumer vertex = consumer.addVertexWith2DPose(this.pose, this.positions.getFloat(i * 2), this.positions.getFloat(i * 2 + 1));
			if (this.textured) {
				vertex.setUv(this.uvs.getFloat(i * 2), this.uvs.getFloat(i * 2 + 1));
			}
			vertex.setColor(this.colors.getInt(i));
		}
	}

	@Override
	public RenderPipeline pipeline() {
		return this.pipeline;
	}

	@Override
	public TextureSetup textureSetup() {
		return this.textureSetup;
	}

	@Override
	public @Nullable ScreenRectangle scissorArea() {
		return null;
	}

	@Override
	public @Nullable ScreenRectangle bounds() {
		return this.bounds;
	}

	private void mapped(float x, float y, int argb, @Nullable UvMap uv) {
		if (uv == null) {
			corner(x, y, argb);
		} else {
			corner(x, y, uv.u(x, y), uv.v(x, y), argb);
		}
	}

	private void corner(float x, float y, int argb) {
		corner(x, y, 0.0F, 0.0F, argb);
	}

	private void corner(float x, float y, float u, float v, int argb) {
		int index = this.pendingCorners;
		this.cornerX[index] = x;
		this.cornerY[index] = y;
		this.cornerU[index] = u;
		this.cornerV[index] = v;
		this.cornerColor[index] = argb;
		this.pendingCorners = index + 1;
		if (this.pendingCorners == CORNERS) {
			this.pendingCorners = 0;
			commitQuad();
		}
	}

	private void commitQuad() {
		float doubledArea = 0.0F;
		for (int i = 0; i < CORNERS; i++) {
			int next = (i + 1) % CORNERS;
			doubledArea += this.cornerX[i] * this.cornerY[next] - this.cornerX[next] * this.cornerY[i];
		}
		int[] order = doubledArea > 0.0F != this.mirrored ? REVERSED : FORWARD;
		for (int index : order) {
			this.positions.add(this.cornerX[index]);
			this.positions.add(this.cornerY[index]);
			this.uvs.add(this.cornerU[index]);
			this.uvs.add(this.cornerV[index]);
			this.colors.add(this.cornerColor[index]);
		}
	}

	private ScreenRectangle computeBounds() {
		float minX = Float.POSITIVE_INFINITY;
		float minY = Float.POSITIVE_INFINITY;
		float maxX = Float.NEGATIVE_INFINITY;
		float maxY = Float.NEGATIVE_INFINITY;
		Vector2f point = new Vector2f();
		for (int i = 0; i < this.colors.size(); i++) {
			this.pose.transformPosition(this.positions.getFloat(i * 2), this.positions.getFloat(i * 2 + 1), point);
			minX = Math.min(minX, point.x);
			minY = Math.min(minY, point.y);
			maxX = Math.max(maxX, point.x);
			maxY = Math.max(maxY, point.y);
		}
		int x = Mth.floor(minX);
		int y = Mth.floor(minY);
		return new ScreenRectangle(x, y, Math.max(1, Mth.ceil(maxX) - x), Math.max(1, Mth.ceil(maxY) - y));
	}

	private static float[] normals(float[] outline, float centerX, float centerY) {
		int count = outline.length / 2;
		float[] normals = new float[outline.length];
		for (int i = 0; i < count; i++) {
			int previous = (i + count - 1) % count;
			int next = (i + 1) % count;
			float x = outline[i * 2];
			float y = outline[i * 2 + 1];
			float nx = edgeNormalX(outline, previous, i) + edgeNormalX(outline, i, next);
			float ny = edgeNormalY(outline, previous, i) + edgeNormalY(outline, i, next);
			float length = Mth.sqrt(nx * nx + ny * ny);
			if (length <= 0.0F) {
				nx = x - centerX;
				ny = y - centerY;
				length = Math.max(Mth.sqrt(nx * nx + ny * ny), 1.0E-6F);
			}
			nx /= length;
			ny /= length;
			if (nx * (x - centerX) + ny * (y - centerY) < 0.0F) {
				nx = -nx;
				ny = -ny;
			}
			normals[i * 2] = nx;
			normals[i * 2 + 1] = ny;
		}
		return normals;
	}

	private static float edgeNormalX(float[] outline, int from, int to) {
		float dy = outline[to * 2 + 1] - outline[from * 2 + 1];
		float dx = outline[to * 2] - outline[from * 2];
		float length = Mth.sqrt(dx * dx + dy * dy);
		return length > 0.0F ? dy / length : 0.0F;
	}

	private static float edgeNormalY(float[] outline, int from, int to) {
		float dy = outline[to * 2 + 1] - outline[from * 2 + 1];
		float dx = outline[to * 2] - outline[from * 2];
		float length = Mth.sqrt(dx * dx + dy * dy);
		return length > 0.0F ? -dx / length : 0.0F;
	}

	private static int transparent(int argb) {
		return argb & 0x00FFFFFF;
	}

	public interface UvMap {
		float u(float x, float y);

		float v(float x, float y);
	}
}
