package sh.rikka.client.render.skin;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.systems.ScissorState;
import com.mojang.renderpearl.api.buffers.GpuBuffer;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import com.mojang.renderpearl.api.commands.RenderPass;
import com.mojang.renderpearl.api.pipeline.PrimitiveTopology;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import it.unimi.dsi.fastutil.ints.Int2IntOpenHashMap;
import java.util.Arrays;
import net.minecraft.client.renderer.oit.OitPipelineSet;
import net.minecraft.client.renderer.oit.OitStage;
import net.minecraft.client.renderer.rendertype.PreparedRenderType;
import org.jspecify.annotations.Nullable;

public final class SkinBatch {
	private static final int BASE_VERTEX = 0;
	private static final int INDEX_COUNT = 1;
	private static final int FIRST_INSTANCE = 2;
	private static final int CAPACITY = 3;
	private static final int COUNT = 4;
	private static final int FIRST_ROW = 5;
	private static final int FIELDS = 6;
	private static final int MAX_CAPACITY = 64;
	private static final int NO_CHUNK = -1;

	private final PreparedRenderType renderType;
	private final boolean ordered;
	private final boolean layered;
	private final Int2IntOpenHashMap openChunks = new Int2IntOpenHashMap();
	private int[] chunks = new int[FIELDS * 4];
	private int chunkCount;

	SkinBatch(PreparedRenderType renderType, boolean ordered, boolean layered) {
		this.renderType = renderType;
		this.ordered = ordered;
		this.layered = layered;
		this.openChunks.defaultReturnValue(NO_CHUNK);
	}

	int reserve(SkinFrame frame, int baseVertex, int indexCount, int rows) {
		int open = this.ordered ? this.chunkCount - 1 : this.openChunks.get(baseVertex);
		int capacity = 1;
		if (open != NO_CHUNK) {
			int offset = open * FIELDS;
			if (this.chunks[offset + BASE_VERTEX] == baseVertex) {
				if (this.chunks[offset + COUNT] < this.chunks[offset + CAPACITY]) {
					return open;
				}
				capacity = Math.min(Math.min(this.chunks[offset + CAPACITY] * 2, MAX_CAPACITY), Math.max(1, SkinLayout.WINDOW_ROWS / rows));
			}
		}
		int chunk = this.chunkCount++;
		int offset = chunk * FIELDS;
		if (offset == this.chunks.length) {
			this.chunks = Arrays.copyOf(this.chunks, offset * 2);
		}
		this.chunks[offset + BASE_VERTEX] = baseVertex;
		this.chunks[offset + INDEX_COUNT] = indexCount;
		this.chunks[offset + FIRST_INSTANCE] = frame.reserveInstances(capacity);
		this.chunks[offset + CAPACITY] = capacity;
		this.chunks[offset + COUNT] = 0;
		this.chunks[offset + FIRST_ROW] = frame.reserveRows(capacity * rows);
		if (!this.ordered) {
			this.openChunks.put(baseVertex, chunk);
		}
		RenderSystem.getSequentialBuffer(PrimitiveTopology.QUADS).requestIndexCount(indexCount);
		return chunk;
	}

	int take(int chunk) {
		return this.chunks[chunk * FIELDS + COUNT]++;
	}

	int firstInstance(int chunk) {
		return this.chunks[chunk * FIELDS + FIRST_INSTANCE];
	}

	int firstRow(int chunk) {
		return this.chunks[chunk * FIELDS + FIRST_ROW];
	}

	void execute(RenderPass renderPass, @Nullable OitStage stage, SkinPipelines pipelines, GpuBufferSlice vertices, GpuBuffer instances, GpuBuffer palette) {
		RenderPipeline source = source(stage);
		renderPass.pushDebugGroup(this.renderType::name);
		if (this.layered && stage == null) {
			draw(renderPass, twin(pipelines, source, SkinFaces.BACK), vertices, instances, palette);
			draw(renderPass, twin(pipelines, source, SkinFaces.FRONT), vertices, instances, palette);
		} else {
			draw(renderPass, twin(pipelines, source, SkinFaces.ALL), vertices, instances, palette);
		}
		renderPass.popDebugGroup();
	}

	private void draw(RenderPass renderPass, RenderPipeline pipeline, GpuBufferSlice vertices, GpuBuffer instances, GpuBuffer palette) {
		RenderSystem.AutoStorageIndexBuffer indices = RenderSystem.getSequentialBuffer(PrimitiveTopology.QUADS);
		renderPass.setPipeline(RenderSystem.getCompiledPipeline(pipeline));
		ScissorState scissor = this.renderType.scissorState();
		if (scissor.enabled()) {
			renderPass.enableScissor(scissor.x(), scissor.y(), scissor.width(), scissor.height());
		}
		RenderSystem.bindDefaultUniforms(renderPass);
		renderPass.setUniform("DynamicTransforms", this.renderType.dynamicTransforms());
		for (PreparedRenderType.Texture texture : this.renderType.textures()) {
			renderPass.setUniform(texture.name(), texture.textureView(), texture.sampler());
		}
		renderPass.setVertexBuffer(0, vertices);
		renderPass.setVertexBuffer(1, instances.slice());
		renderPass.setIndexBuffer(indices.getBuffer(), indices.type());
		for (int offset = 0; offset < this.chunkCount * FIELDS; offset += FIELDS) {
			renderPass.setUniform(SkinLayout.PALETTE_UNIFORM, palette.slice((long) this.chunks[offset + FIRST_ROW] * SkinLayout.ROW_BYTES, SkinLayout.WINDOW_BYTES));
			renderPass.drawIndexed(this.chunks[offset + INDEX_COUNT], this.chunks[offset + COUNT], 0, this.chunks[offset + BASE_VERTEX], this.chunks[offset + FIRST_INSTANCE]);
		}
	}

	private RenderPipeline twin(SkinPipelines pipelines, RenderPipeline source, SkinFaces faces) {
		RenderPipeline twin = pipelines.twin(source, faces);
		if (twin == null) {
			throw new IllegalStateException("render type " + this.renderType.name() + " has no skin pipeline");
		}
		return twin;
	}

	private RenderPipeline source(@Nullable OitStage stage) {
		if (stage == null) {
			return this.renderType.pipeline();
		}
		OitPipelineSet set = this.renderType.oitPipelineSet();
		if (set == null) {
			throw new IllegalStateException("render type " + this.renderType.name() + " has no OIT pipelines");
		}
		return set.getPipeline(stage);
	}
}
