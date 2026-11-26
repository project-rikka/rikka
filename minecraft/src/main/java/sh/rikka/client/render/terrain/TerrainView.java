package sh.rikka.client.render.terrain;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import com.mojang.renderpearl.api.commands.RenderPass;
import com.mojang.renderpearl.api.pipeline.IndexType;
import com.mojang.renderpearl.api.pipeline.PrimitiveTopology;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import com.mojang.renderpearl.api.textures.FilterMode;
import com.mojang.renderpearl.api.textures.GpuSampler;
import com.mojang.renderpearl.api.textures.GpuTextureView;
import com.mojang.renderpearl.backend.vulkan.VulkanDevice;
import it.unimi.dsi.fastutil.longs.LongArrayList;
import it.unimi.dsi.fastutil.objects.ObjectArrayList;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.client.renderer.ShaderDefines;
import net.minecraft.client.renderer.chunk.ChunkSectionLayer;
import net.minecraft.client.renderer.chunk.ChunkSectionLayerGroup;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import net.minecraft.client.renderer.oit.OitRenderPassProvider;
import net.minecraft.client.renderer.oit.OitStage;
import net.minecraft.world.phys.Vec3;
import org.joml.Matrix4f;
import org.joml.Matrix4fc;
import org.jspecify.annotations.Nullable;
import org.lwjgl.system.MemoryStack;
import org.lwjgl.vulkan.VK10;
import org.lwjgl.vulkan.VkCommandBuffer;
import org.lwjgl.vulkan.VkDescriptorBufferInfo;
import org.lwjgl.vulkan.VkWriteDescriptorSet;
import sh.rikka.client.mixin.VulkanRenderPassAccessor;
import sh.rikka.client.mods.boost.BoostFeature;
import sh.rikka.client.render.vulkan.ComputePipeline;
import sh.rikka.client.render.vulkan.DeviceBuffer;
import sh.rikka.client.render.vulkan.VulkanCommands;

public final class TerrainView implements AutoCloseable {
	static final int[] CULL_DESCRIPTORS = {
		VK10.VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
		VK10.VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
		VK10.VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
		VK10.VK_DESCRIPTOR_TYPE_STORAGE_BUFFER
	};
	static final int CULL_CONSTANT_BYTES = 96;

	private static final String OIT_PASS = "Terrain";
	private static final int LAYER_SHIFT = 28;
	private static final int MAX_SLOTS = 1 << LAYER_SHIFT;
	private static final int DRAWS_PER_ITEM = 3;
	private static final int DRAW_BYTES = 20;
	private static final int WORKGROUP = 64;
	private static final int INITIAL_ITEMS = 1 << 12;
	private static final int CAMERA_BLOCK_OFFSET = 64;
	private static final int CAMERA_OFFSET_OFFSET = 80;
	private static final int OPAQUE_FOG_DISTANCE_OFFSET = 92;
	private static final ChunkSectionLayer[] LAYERS = ChunkSectionLayer.values();

	static final ShaderDefines CULL_DEFINES = ShaderDefines.builder()
		.define("WORKGROUP", WORKGROUP)
		.define("LAYER_SHIFT", LAYER_SHIFT)
		.define("RECORD_WORDS", SectionTable.RECORD_WORDS)
		.define("LAYER_WORDS", SectionTable.LAYER_WORDS)
		.define("BUCKETS", SectionTable.BUCKETS)
		.define("UNASSIGNED", SectionTable.UNASSIGNED_BUCKET)
		.define("INDICES_PER_QUAD", SectionTable.INDICES_PER_QUAD)
		.define("DRAWS_PER_ITEM", DRAWS_PER_ITEM)
		.define("DRAW_WORDS", DRAW_BYTES / Integer.BYTES)
		.build();

	private final VulkanDevice device;
	private final List<List<Batch>> batches = new ArrayList<>();
	private final LongArrayList keys = new LongArrayList();
	private final Matrix4f viewProjection = new Matrix4f();
	private final ByteBuffer constants = ByteBuffer.allocateDirect(CULL_CONSTANT_BYTES).order(ByteOrder.nativeOrder());
	private int[] items = new int[INITIAL_ITEMS];
	private int itemCount;
	private int uploadedCapacity;
	private @Nullable DeviceBuffer itemBuffer;
	private @Nullable DeviceBuffer drawBuffer;
	private int builtVisible = -1;
	private int builtGeometry = -1;
	private boolean builtCompact;
	private @Nullable GpuBufferSlice transform;
	private long instanceBuffer;

	TerrainView(VulkanDevice device) {
		this.device = device;
		for (int layer = 0; layer < LAYERS.length; layer++) {
			this.batches.add(new ArrayList<>());
		}
	}

	void prepare(
		SectionTable table,
		ComputePipeline cull,
		ObjectArrayList<SectionRenderDispatcher.RenderSection> visible,
		int visibleVersion,
		Matrix4fc projection,
		Matrix4fc viewRotation,
		Vec3 camera,
		float opaqueFogDistance,
		GpuBufferSlice transform
	) {
		this.transform = transform;
		VkCommandBuffer commands = VulkanCommands.outsidePass();
		boolean compact = BoostFeature.COMPACT_VERTICES.isActive();
		if (this.builtVisible != visibleVersion || this.builtGeometry != table.geometryVersion() || this.builtCompact != compact) {
			this.builtVisible = visibleVersion;
			this.builtGeometry = table.geometryVersion();
			this.builtCompact = compact;
			this.build(table, visible);
			this.uploadItems(commands);
		}
		VulkanCommands.barrier(commands);
		if (this.itemCount == 0) {
			return;
		}
		this.instanceBuffer = table.instanceBuffer().handle();
		this.writeConstants(projection, viewRotation, camera, opaqueFogDistance);
		try (MemoryStack stack = MemoryStack.stackPush()) {
			VkWriteDescriptorSet.Buffer writes = VkWriteDescriptorSet.calloc(CULL_DESCRIPTORS.length, stack);
			DeviceBuffer[] buffers = {table.instanceBuffer(), table.recordBuffer(), require(this.itemBuffer), require(this.drawBuffer)};
			for (int binding = 0; binding < buffers.length; binding++) {
				VkDescriptorBufferInfo.Buffer info = VkDescriptorBufferInfo.calloc(1, stack)
					.buffer(buffers[binding].handle())
					.offset(0L)
					.range(VK10.VK_WHOLE_SIZE);
				writes.get(binding)
					.sType$Default()
					.dstBinding(binding)
					.descriptorCount(1)
					.descriptorType(VK10.VK_DESCRIPTOR_TYPE_STORAGE_BUFFER)
					.pBufferInfo(info);
			}
			cull.bind(commands);
			cull.pushDescriptors(commands, writes);
			cull.pushConstants(commands, this.constants);
			VK10.vkCmdDispatch(commands, (this.itemCount + WORKGROUP - 1) / WORKGROUP, 1, 1);
		}
		VulkanCommands.barrier(commands);
	}

	public void draw(ChunkSectionLayerGroup group, RenderPass pass, GpuSampler sampler, GpuTextureView atlas, boolean wireframe) {
		GpuTextureView lightmap = Minecraft.getInstance().gameRenderer.lightmap();
		if (wireframe) {
			RenderPipeline pipeline = TerrainPipelines.of(RenderPipelines.WIREFRAME_MULTIDRAW, this.builtCompact, false);
			this.draw(group.layers(), pass, sampler, atlas, lightmap, layer -> pipeline);
		} else {
			boolean fastFiltering = BoostFeature.FAST_FILTERING.isActive();
			this.draw(group.layers(), pass, sampler, atlas, lightmap, layer -> TerrainPipelines.of(layer.pipeline(true), this.builtCompact, fastFiltering));
		}
	}

	public void drawOit(GpuSampler sampler, OitStage stage, OitRenderPassProvider.Parameters parameters, GpuTextureView atlas, GpuTextureView lightmap) {
		try (RenderPass pass = OitRenderPassProvider.createRenderPass(stage, () -> OIT_PASS, parameters)) {
			RenderPipeline vanilla = RenderPipelines.OIT_TERRAIN_MULTIDRAW.getPipeline(stage);
			RenderPipeline pipeline = TerrainPipelines.of(vanilla, this.builtCompact, BoostFeature.FAST_FILTERING.isActive());
			this.draw(ChunkSectionLayerGroup.TRANSLUCENT.layers(), pass, sampler, atlas, lightmap, layer -> pipeline);
		}
	}

	@Override
	public void close() {
		if (this.itemBuffer != null) {
			this.itemBuffer.close();
			this.itemBuffer = null;
		}
		if (this.drawBuffer != null) {
			this.drawBuffer.close();
			this.drawBuffer = null;
		}
		this.uploadedCapacity = 0;
		this.builtVisible = -1;
		this.itemCount = 0;
		this.batches.forEach(List::clear);
	}

	private void draw(
		ChunkSectionLayer[] layers,
		RenderPass pass,
		GpuSampler sampler,
		GpuTextureView atlas,
		GpuTextureView lightmap,
		Function<ChunkSectionLayer, RenderPipeline> pipelines
	) {
		if (this.itemCount == 0) {
			return;
		}
		RenderSystem.AutoStorageIndexBuffer sequential = RenderSystem.getSequentialBuffer(PrimitiveTopology.QUADS);
		pass.setUniform("TerrainUniform", require(this.transform));
		pass.setUniform("Sampler0", atlas, sampler);
		pass.setUniform("Sampler2", lightmap, RenderSystem.getSamplerCache().getClampToEdge(FilterMode.LINEAR));
		int limit = RenderSystem.getDevice().getDeviceInfo().limits().maxDrawIndirectDrawCount();
		long draws = require(this.drawBuffer).handle();
		for (ChunkSectionLayer layer : layers) {
			List<Batch> layerBatches = this.batches.get(layer.ordinal());
			if (layerBatches.isEmpty()) {
				continue;
			}
			pass.setPipeline(RenderSystem.getCompiledPipeline(pipelines.apply(layer)));
			VulkanRenderPassAccessor raw = VulkanCommands.inside(pass);
			raw.rikka$pushDescriptors();
			VkCommandBuffer commands = raw.rikka$commandBuffer();
			try (MemoryStack stack = MemoryStack.stackPush()) {
				VK10.vkCmdBindVertexBuffers(commands, 1, stack.longs(this.instanceBuffer), stack.longs(0L));
				for (Batch batch : layerBatches) {
					VK10.vkCmdBindVertexBuffers(commands, 0, stack.longs(batch.vertexBuffer), stack.longs(0L));
					if (batch.indexBuffer != 0L) {
						VK10.vkCmdBindIndexBuffer(commands, batch.indexBuffer, 0L, batch.wideIndices ? VK10.VK_INDEX_TYPE_UINT32 : VK10.VK_INDEX_TYPE_UINT16);
					} else {
						int type = sequential.type() == IndexType.INT ? VK10.VK_INDEX_TYPE_UINT32 : VK10.VK_INDEX_TYPE_UINT16;
						VK10.vkCmdBindIndexBuffer(commands, VulkanCommands.handle(sequential.getBuffer()), 0L, type);
					}
					int remaining = batch.itemCount * DRAWS_PER_ITEM;
					long offset = (long) batch.firstItem * DRAWS_PER_ITEM * DRAW_BYTES;
					while (remaining > 0) {
						int count = Math.min(remaining, limit);
						VK10.vkCmdDrawIndexedIndirect(commands, draws, offset, count, DRAW_BYTES);
						remaining -= count;
						offset += (long) count * DRAW_BYTES;
					}
				}
			}
		}
	}

	private void build(SectionTable table, ObjectArrayList<SectionRenderDispatcher.RenderSection> visible) {
		this.itemCount = 0;
		this.batches.forEach(List::clear);
		int needed = visible.size() * LAYERS.length;
		if (this.items.length < needed) {
			this.items = new int[Integer.highestOneBit(needed) * 2];
		}
		for (ChunkSectionLayer layer : LAYERS) {
			if (layer.translucent()) {
				this.buildOrdered(table, visible, layer.ordinal());
			} else {
				this.buildGrouped(table, visible, layer.ordinal());
			}
		}
	}

	private void buildGrouped(SectionTable table, ObjectArrayList<SectionRenderDispatcher.RenderSection> visible, int layer) {
		List<Batch> layerBatches = this.batches.get(layer);
		this.keys.clear();
		for (int index = 0; index < visible.size(); index++) {
			long vertexBuffer = this.vertexBuffer(table, slot(visible.get(index)), layer);
			if (vertexBuffer == 0L) {
				continue;
			}
			int key = this.keys.indexOf(vertexBuffer);
			if (key < 0) {
				this.keys.add(vertexBuffer);
				layerBatches.add(new Batch(vertexBuffer, 0L, false));
			} else {
				layerBatches.get(key).itemCount++;
			}
		}
		int first = this.itemCount;
		for (Batch batch : layerBatches) {
			batch.firstItem = first;
			first += batch.itemCount;
			batch.itemCount = 0;
		}
		this.itemCount = first;
		for (int index = 0; index < visible.size(); index++) {
			int slot = visible.get(index).index;
			long vertexBuffer = this.vertexBuffer(table, slot, layer);
			if (vertexBuffer != 0L) {
				Batch batch = layerBatches.get(this.keys.indexOf(vertexBuffer));
				this.items[batch.firstItem + batch.itemCount++] = slot | layer << LAYER_SHIFT;
			}
		}
	}

	private void buildOrdered(SectionTable table, ObjectArrayList<SectionRenderDispatcher.RenderSection> visible, int layer) {
		List<Batch> layerBatches = this.batches.get(layer);
		Batch batch = null;
		for (int index = visible.size() - 1; index >= 0; index--) {
			int slot = slot(visible.get(index));
			long vertexBuffer = this.vertexBuffer(table, slot, layer);
			if (vertexBuffer == 0L) {
				continue;
			}
			long indexBuffer = table.indexBuffer(slot, layer);
			boolean wide = table.wideIndices(slot, layer);
			if (batch == null || batch.vertexBuffer != vertexBuffer || batch.indexBuffer != indexBuffer || batch.wideIndices != wide) {
				batch = new Batch(vertexBuffer, indexBuffer, wide);
				batch.firstItem = this.itemCount;
				batch.itemCount = 0;
				layerBatches.add(batch);
			}
			batch.itemCount++;
			this.items[this.itemCount++] = slot | layer << LAYER_SHIFT;
		}
	}

	private long vertexBuffer(SectionTable table, int slot, int layer) {
		return table.compactVertices(slot, layer) == this.builtCompact ? table.vertexBuffer(slot, layer) : 0L;
	}

	private static int slot(SectionRenderDispatcher.RenderSection section) {
		if (section.index >= MAX_SLOTS) {
			throw new IllegalStateException("the view area holds more than " + MAX_SLOTS + " sections");
		}
		return section.index;
	}

	private void uploadItems(VkCommandBuffer commands) {
		if (this.uploadedCapacity < this.items.length) {
			this.uploadedCapacity = this.items.length;
			if (this.itemBuffer != null) {
				this.itemBuffer.close();
				require(this.drawBuffer).close();
			}
			this.itemBuffer = new DeviceBuffer(
				this.device,
				(long) this.uploadedCapacity * Integer.BYTES,
				VK10.VK_BUFFER_USAGE_STORAGE_BUFFER_BIT | VK10.VK_BUFFER_USAGE_TRANSFER_DST_BIT
			);
			this.drawBuffer = new DeviceBuffer(
				this.device,
				(long) this.uploadedCapacity * DRAWS_PER_ITEM * DRAW_BYTES,
				VK10.VK_BUFFER_USAGE_STORAGE_BUFFER_BIT | VK10.VK_BUFFER_USAGE_INDIRECT_BUFFER_BIT
			);
		}
		if (this.itemCount == 0) {
			return;
		}
		long bytes = (long) this.itemCount * Integer.BYTES;
		try (VulkanCommands.Staged staged = VulkanCommands.stage(bytes)) {
			staged.data().order(ByteOrder.nativeOrder()).asIntBuffer().put(this.items, 0, this.itemCount);
			staged.copyTo(commands, require(this.itemBuffer), 0L, bytes);
		}
	}

	private void writeConstants(Matrix4fc projection, Matrix4fc viewRotation, Vec3 camera, float opaqueFogDistance) {
		projection.mul(viewRotation, this.viewProjection);
		this.viewProjection.get(0, this.constants);
		int blockX = (int) Math.floor(camera.x);
		int blockY = (int) Math.floor(camera.y);
		int blockZ = (int) Math.floor(camera.z);
		this.constants
			.putInt(CAMERA_BLOCK_OFFSET, blockX)
			.putInt(CAMERA_BLOCK_OFFSET + 4, blockY)
			.putInt(CAMERA_BLOCK_OFFSET + 8, blockZ)
			.putInt(CAMERA_BLOCK_OFFSET + 12, this.itemCount)
			.putFloat(CAMERA_OFFSET_OFFSET, (float) (blockX - camera.x))
			.putFloat(CAMERA_OFFSET_OFFSET + 4, (float) (blockY - camera.y))
			.putFloat(CAMERA_OFFSET_OFFSET + 8, (float) (blockZ - camera.z))
			.putFloat(OPAQUE_FOG_DISTANCE_OFFSET, opaqueFogDistance);
	}

	private static <T> T require(@Nullable T value) {
		if (value == null) {
			throw new IllegalStateException("the terrain view was not prepared");
		}
		return value;
	}

	private static final class Batch {
		final long vertexBuffer;
		final long indexBuffer;
		final boolean wideIndices;
		int firstItem;
		int itemCount = 1;

		Batch(long vertexBuffer, long indexBuffer, boolean wideIndices) {
			this.vertexBuffer = vertexBuffer;
			this.indexBuffer = indexBuffer;
			this.wideIndices = wideIndices;
		}
	}
}
