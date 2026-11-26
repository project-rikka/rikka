package sh.rikka.client.render.skin;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.renderpearl.api.buffers.GpuBuffer;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import com.mojang.renderpearl.api.commands.RenderPass;
import it.unimi.dsi.fastutil.objects.Reference2ReferenceOpenHashMap;
import net.minecraft.client.renderer.StagedVertexBuffer;
import net.minecraft.client.renderer.oit.OitStage;
import net.minecraft.client.renderer.rendertype.PreparedRenderType;
import net.minecraft.util.Mth;
import org.jspecify.annotations.Nullable;
import org.lwjgl.system.MemoryUtil;

public final class SkinFrame implements AutoCloseable {
	private final Skins skins;
	private final int rowAlignment;
	private final Scratch instances = new Scratch(SkinLayout.INSTANCE_BYTES);
	private final Scratch rows = new Scratch(SkinLayout.ROW_BYTES);
	private final Reference2ReferenceOpenHashMap<StagedVertexBuffer.Draw, SkinBatch> batches = new Reference2ReferenceOpenHashMap<>();
	private final TransientBuffers instanceBuffers = new TransientBuffers("Rikka Skin Instances", GpuBuffer.USAGE_VERTEX);
	private final TransientBuffers paletteBuffers = new TransientBuffers("Rikka Skin Palette", GpuBuffer.USAGE_UNIFORM);
	private @Nullable GpuBuffer instanceBuffer;
	private @Nullable GpuBuffer paletteBuffer;

	public SkinFrame(Skins skins) {
		this.skins = skins;
		int alignment = RenderSystem.getDevice().getDeviceInfo().limits().minUniformOffsetAlignment();
		this.rowAlignment = Math.ceilDiv(alignment, SkinLayout.ROW_BYTES);
	}

	public Skins skins() {
		return this.skins;
	}

	public SkinBatch createBatch(PreparedRenderType renderType, StagedVertexBuffer.Draw placeholder, boolean ordered, boolean layered) {
		SkinBatch batch = new SkinBatch(renderType, ordered, layered);
		this.batches.put(placeholder, batch);
		return batch;
	}

	public long append(SkinBatch batch, int baseVertex, int indexCount, int rows, int abgr, int overlayCoords, int lightCoords, UvTransform uv) {
		int chunk = batch.reserve(this, baseVertex, indexCount, rows);
		int slot = batch.take(chunk);
		long instance = this.instances.address(batch.firstInstance(chunk) + slot);
		MemoryUtil.memPutInt(instance, abgr);
		MemoryUtil.memPutInt(instance + SkinLayout.INSTANCE_OVERLAY_OFFSET, overlayCoords);
		MemoryUtil.memPutInt(instance + SkinLayout.INSTANCE_LIGHT_OFFSET, lightCoords);
		MemoryUtil.memPutInt(instance + SkinLayout.INSTANCE_BASE_OFFSET, slot * rows);
		MemoryUtil.memPutFloat(instance + SkinLayout.INSTANCE_UV_OFFSET, uv.uScale());
		MemoryUtil.memPutFloat(instance + SkinLayout.INSTANCE_UV_OFFSET + 4L, uv.vScale());
		MemoryUtil.memPutFloat(instance + SkinLayout.INSTANCE_UV_OFFSET + 8L, uv.uOffset());
		MemoryUtil.memPutFloat(instance + SkinLayout.INSTANCE_UV_OFFSET + 12L, uv.vOffset());
		return this.rows.address(batch.firstRow(chunk) + slot * rows);
	}

	int reserveInstances(int count) {
		return this.instances.reserve(count, 1);
	}

	int reserveRows(int count) {
		return this.rows.reserve(count, this.rowAlignment);
	}

	public void upload() {
		this.skins.meshes().upload();
		if (this.instances.count() == 0) {
			return;
		}
		this.instanceBuffer = this.instances.copyTo(this.instanceBuffers.acquire(this.instances.bytes()));
		this.paletteBuffer = this.rows.copyTo(this.paletteBuffers.acquire(this.rows.bytes() + SkinLayout.WINDOW_BYTES));
	}

	public boolean execute(StagedVertexBuffer.Draw placeholder, RenderPass renderPass, @Nullable OitStage stage) {
		SkinBatch batch = this.batches.get(placeholder);
		if (batch == null) {
			return false;
		}
		GpuBuffer instances = this.instanceBuffer;
		GpuBuffer palette = this.paletteBuffer;
		if (instances == null || palette == null) {
			throw new IllegalStateException("skin frame is not uploaded");
		}
		GpuBufferSlice vertices = this.skins.meshes().vertices();
		batch.execute(renderPass, stage, this.skins.pipelines(), vertices, instances, palette);
		return true;
	}

	public void endDraw() {
		this.batches.clear();
		this.instances.clear();
		this.rows.clear();
		this.instanceBuffer = null;
		this.paletteBuffer = null;
	}

	public void endFrame() {
		this.instanceBuffers.endFrame();
		this.paletteBuffers.endFrame();
	}

	@Override
	public void close() {
		this.instanceBuffers.close();
		this.paletteBuffers.close();
		this.instances.close();
		this.rows.close();
	}

	private static final class Scratch implements AutoCloseable {
		private static final int INITIAL_ELEMENTS = 1024;

		private final int elementBytes;
		private long address;
		private int capacity = INITIAL_ELEMENTS;
		private int count;

		private Scratch(int elementBytes) {
			this.elementBytes = elementBytes;
			this.address = MemoryUtil.nmemAlloc((long) INITIAL_ELEMENTS * elementBytes);
		}

		private int reserve(int elements, int alignment) {
			int first = Mth.roundToward(this.count, alignment);
			int end = first + elements;
			if (end > this.capacity) {
				this.capacity = Mth.smallestEncompassingPowerOfTwo(end);
				this.address = MemoryUtil.nmemRealloc(this.address, (long) this.capacity * this.elementBytes);
			}
			this.count = end;
			return first;
		}

		private long address(int element) {
			return this.address + (long) element * this.elementBytes;
		}

		private int count() {
			return this.count;
		}

		private long bytes() {
			return (long) this.count * this.elementBytes;
		}

		private GpuBuffer copyTo(TransientBuffers.Mapped target) {
			MemoryUtil.memCopy(this.address, target.address(), bytes());
			return target.buffer();
		}

		private void clear() {
			this.count = 0;
		}

		@Override
		public void close() {
			MemoryUtil.nmemFree(this.address);
			this.address = MemoryUtil.NULL;
		}
	}
}
