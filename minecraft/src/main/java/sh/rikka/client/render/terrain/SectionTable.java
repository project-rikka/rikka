package sh.rikka.client.render.terrain;

import com.mojang.renderpearl.backend.vulkan.VulkanDevice;
import it.unimi.dsi.fastutil.ints.IntArrayList;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.IntBuffer;
import java.util.Arrays;
import net.minecraft.client.renderer.chunk.ChunkSectionLayer;
import net.minecraft.client.renderer.chunk.SectionMesh;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import net.minecraft.core.BlockPos;
import net.minecraft.util.Mth;
import org.jspecify.annotations.Nullable;
import org.lwjgl.system.MemoryStack;
import org.lwjgl.vulkan.VK10;
import org.lwjgl.vulkan.VkBufferCopy;
import org.lwjgl.vulkan.VkCommandBuffer;
import sh.rikka.client.render.vulkan.DeviceBuffer;
import sh.rikka.client.render.vulkan.VulkanCommands;

final class SectionTable implements AutoCloseable {
	static final int LAYERS = ChunkSectionLayer.values().length;
	static final int BUCKETS = 7;
	static final int UNASSIGNED_BUCKET = 3;
	static final int INSTANCE_BYTES = 16;
	static final int INDICES_PER_QUAD = 6;
	static final int LAYER_WORDS = 2 + BUCKETS;

	private static final int RECORD_ALIGNMENT_WORDS = 4;
	static final int RECORD_WORDS = Mth.roundToward(LAYERS * LAYER_WORDS, RECORD_ALIGNMENT_WORDS);

	private static final int INSTANCE_WORDS = INSTANCE_BYTES / Integer.BYTES;
	private static final int RECORD_BYTES = RECORD_WORDS * Integer.BYTES;
	private static final int VISIBILITY_WORD = 3;
	private static final int INITIAL_SLOTS = 1 << 12;
	private static final int STACK_REGIONS = 256;
	private static final int OPAQUE = Float.floatToRawIntBits(1.0F);
	private static final ChunkSectionLayer[] LAYER_VALUES = ChunkSectionLayer.values();

	private final VulkanDevice device;
	private final Object marks = new Object();
	private final IntArrayList marked = new IntArrayList();
	private final IntArrayList flushing = new IntArrayList();
	private final IntArrayList fading = new IntArrayList();
	private SectionRenderDispatcher.@Nullable RenderSection[] sections = new SectionRenderDispatcher.RenderSection[INITIAL_SLOTS];
	private boolean[] isMarked = new boolean[INITIAL_SLOTS];
	private boolean[] isFading = new boolean[INITIAL_SLOTS];
	private int[] instances = new int[INITIAL_SLOTS * INSTANCE_WORDS];
	private int[] records = new int[INITIAL_SLOTS * RECORD_WORDS];
	private long[] vertexBuffers = new long[INITIAL_SLOTS * LAYERS];
	private long[] indexBuffers = new long[INITIAL_SLOTS * LAYERS];
	private boolean[] wideIndices = new boolean[INITIAL_SLOTS * LAYERS];
	private boolean[] compactVertices = new boolean[INITIAL_SLOTS * LAYERS];
	private @Nullable DeviceBuffer instanceBuffer;
	private @Nullable DeviceBuffer recordBuffer;
	private int uploadedSlots;
	private int geometryVersion;
	private int largestSequentialIndexCount;

	SectionTable(VulkanDevice device) {
		this.device = device;
	}

	void beginEpoch() {
		synchronized (this.marks) {
			Arrays.fill(this.sections, null);
			Arrays.fill(this.isMarked, false);
			this.marked.clear();
		}
		Arrays.fill(this.isFading, false);
		Arrays.fill(this.vertexBuffers, 0L);
		Arrays.fill(this.records, 0);
		this.fading.clear();
		this.uploadedSlots = 0;
		this.geometryVersion++;
	}

	void register(SectionRenderDispatcher.RenderSection section) {
		synchronized (this.marks) {
			if (section.index >= this.sections.length) {
				this.grow(Math.max(section.index + 1, this.sections.length * 2));
			}
			this.sections[section.index] = section;
		}
		this.mark(section);
	}

	void mark(SectionRenderDispatcher.RenderSection section) {
		synchronized (this.marks) {
			int slot = section.index;
			if (slot < this.sections.length && this.sections[slot] == section && !this.isMarked[slot]) {
				this.isMarked[slot] = true;
				this.marked.add(slot);
			}
		}
	}

	void flush(SectionRenderDispatcher dispatcher, long now, long fadeMillis) {
		int slots;
		synchronized (this.marks) {
			slots = this.sections.length;
			this.flushing.clear();
			this.flushing.addAll(this.marked);
			for (int index = 0; index < this.marked.size(); index++) {
				this.isMarked[this.marked.getInt(index)] = false;
			}
			this.marked.clear();
		}
		dispatcher.lock();
		try {
			for (int index = 0; index < this.flushing.size(); index++) {
				this.refresh(dispatcher, this.flushing.getInt(index), now, fadeMillis);
			}
		} finally {
			dispatcher.unlock();
		}
		int steady = this.flushing.size();
		for (int index = this.fading.size() - 1; index >= 0; index--) {
			int slot = this.fading.getInt(index);
			SectionRenderDispatcher.RenderSection section = this.section(slot);
			float visibility = section == null ? 1.0F : section.getVisibility(now, fadeMillis);
			this.instances[slot * INSTANCE_WORDS + VISIBILITY_WORD] = Float.floatToRawIntBits(visibility);
			this.flushing.add(slot);
			if (visibility >= 1.0F) {
				this.isFading[slot] = false;
				this.fading.set(index, this.fading.getInt(this.fading.size() - 1));
				this.fading.removeInt(this.fading.size() - 1);
			}
		}
		this.upload(slots, steady);
	}

	int geometryVersion() {
		return this.geometryVersion;
	}

	int largestSequentialIndexCount() {
		return this.largestSequentialIndexCount;
	}

	long vertexBuffer(int slot, int layer) {
		return this.vertexBuffers[slot * LAYERS + layer];
	}

	long indexBuffer(int slot, int layer) {
		return this.indexBuffers[slot * LAYERS + layer];
	}

	boolean wideIndices(int slot, int layer) {
		return this.wideIndices[slot * LAYERS + layer];
	}

	boolean compactVertices(int slot, int layer) {
		return this.compactVertices[slot * LAYERS + layer];
	}

	DeviceBuffer instanceBuffer() {
		return require(this.instanceBuffer);
	}

	DeviceBuffer recordBuffer() {
		return require(this.recordBuffer);
	}

	@Override
	public void close() {
		if (this.instanceBuffer != null) {
			this.instanceBuffer.close();
			this.instanceBuffer = null;
		}
		if (this.recordBuffer != null) {
			this.recordBuffer.close();
			this.recordBuffer = null;
		}
	}

	private SectionRenderDispatcher.@Nullable RenderSection section(int slot) {
		synchronized (this.marks) {
			return slot < this.sections.length ? this.sections[slot] : null;
		}
	}

	private void refresh(SectionRenderDispatcher dispatcher, int slot, long now, long fadeMillis) {
		SectionRenderDispatcher.RenderSection section = this.section(slot);
		if (section == null) {
			return;
		}
		SectionMesh mesh = section.getSectionMesh();
		BlockPos origin = section.getRenderOrigin();
		int instance = slot * INSTANCE_WORDS;
		this.instances[instance] = origin.getX();
		this.instances[instance + 1] = origin.getY();
		this.instances[instance + 2] = origin.getZ();
		this.instances[instance + VISIBILITY_WORD] = OPAQUE;
		boolean drawn = false;
		for (ChunkSectionLayer layer : LAYER_VALUES) {
			drawn |= this.refreshLayer(dispatcher, slot, mesh, layer);
		}
		if (drawn && !this.isFading[slot] && section.getVisibility(now, fadeMillis) < 1.0F) {
			this.isFading[slot] = true;
			this.fading.add(slot);
		}
	}

	private boolean refreshLayer(SectionRenderDispatcher dispatcher, int slot, SectionMesh mesh, ChunkSectionLayer layer) {
		int key = slot * LAYERS + layer.ordinal();
		int record = slot * RECORD_WORDS + layer.ordinal() * LAYER_WORDS;
		SectionMesh.SectionDraw draw = mesh.getSectionDraw(layer);
		SectionRenderDispatcher.RenderSectionBufferSlice slice = draw == null ? null : dispatcher.getRenderSectionSlice(mesh, layer);
		long vertexBuffer = 0L;
		long indexBuffer = 0L;
		boolean wide = false;
		boolean compact = mesh instanceof CompactMesh compactMesh && compactMesh.rikka$compact();
		int vertexSize = compact ? CompactVertices.STRIDE : layer.vertexFormat().getVertexSize();
		Arrays.fill(this.records, record, record + LAYER_WORDS, 0);
		if (draw != null && slice != null && (!draw.hasCustomIndexBuffer() || slice.indexBuffer() != null) && slice.vertexBufferOffset() % vertexSize == 0) {
			vertexBuffer = VulkanCommands.handle(slice.vertexBuffer());
			this.records[record] = (int) (slice.vertexBufferOffset() / vertexSize);
			if (slice.indexBuffer() != null) {
				indexBuffer = VulkanCommands.handle(slice.indexBuffer());
				wide = draw.indexType().bytes == Integer.BYTES;
				this.records[record + 1] = (int) (slice.indexBufferOffset() / draw.indexType().bytes);
			} else {
				this.largestSequentialIndexCount = Math.max(this.largestSequentialIndexCount, draw.indexCount());
			}
			int quads = draw.indexCount() / INDICES_PER_QUAD;
			int[] ends = mesh instanceof BucketedMesh bucketed ? bucketed.rikka$bucketEnds(layer) : null;
			for (int bucket = 0; bucket < BUCKETS; bucket++) {
				this.records[record + 2 + bucket] = ends != null ? ends[bucket] : bucket >= UNASSIGNED_BUCKET ? quads : 0;
			}
		}
		if (this.vertexBuffers[key] != vertexBuffer || this.indexBuffers[key] != indexBuffer || this.wideIndices[key] != wide || this.compactVertices[key] != compact) {
			this.vertexBuffers[key] = vertexBuffer;
			this.indexBuffers[key] = indexBuffer;
			this.wideIndices[key] = wide;
			this.compactVertices[key] = compact;
			this.geometryVersion++;
		}
		return vertexBuffer != 0L;
	}

	private void upload(int slots, int steady) {
		if (this.uploadedSlots < slots) {
			this.close();
			int usage = VK10.VK_BUFFER_USAGE_STORAGE_BUFFER_BIT | VK10.VK_BUFFER_USAGE_TRANSFER_DST_BIT;
			this.instanceBuffer = new DeviceBuffer(this.device, (long) slots * INSTANCE_BYTES, usage | VK10.VK_BUFFER_USAGE_VERTEX_BUFFER_BIT);
			this.recordBuffer = new DeviceBuffer(this.device, (long) slots * RECORD_BYTES, usage);
			this.uploadedSlots = slots;
			VkCommandBuffer commands = VulkanCommands.outsidePass();
			copyAll(commands, this.instances, slots * INSTANCE_WORDS, this.instanceBuffer());
			copyAll(commands, this.records, slots * RECORD_WORDS, this.recordBuffer());
			return;
		}
		int changed = this.flushing.size();
		if (changed == 0) {
			return;
		}
		VkCommandBuffer commands = VulkanCommands.outsidePass();
		this.copySlots(commands, this.instances, INSTANCE_WORDS, changed, this.instanceBuffer());
		if (steady > 0) {
			this.copySlots(commands, this.records, RECORD_WORDS, steady, this.recordBuffer());
		}
	}

	private void copySlots(VkCommandBuffer commands, int[] source, int words, int count, DeviceBuffer target) {
		long bytes = (long) words * Integer.BYTES;
		boolean onStack = count <= STACK_REGIONS;
		try (VulkanCommands.Staged staged = VulkanCommands.stage(count * bytes); MemoryStack stack = MemoryStack.stackPush()) {
			IntBuffer data = staged.data().order(ByteOrder.nativeOrder()).asIntBuffer();
			VkBufferCopy.Buffer regions = onStack ? VkBufferCopy.calloc(count, stack) : VkBufferCopy.calloc(count);
			try {
				for (int index = 0; index < count; index++) {
					int slot = this.flushing.getInt(index);
					data.put(source, slot * words, words);
					regions.get(index).srcOffset(index * bytes).dstOffset(slot * bytes).size(bytes);
				}
				staged.copyTo(commands, target, regions);
			} finally {
				if (!onStack) {
					regions.free();
				}
			}
		}
	}

	private static void copyAll(VkCommandBuffer commands, int[] source, int words, DeviceBuffer target) {
		long bytes = (long) words * Integer.BYTES;
		try (VulkanCommands.Staged staged = VulkanCommands.stage(bytes)) {
			ByteBuffer data = staged.data().order(ByteOrder.nativeOrder());
			data.asIntBuffer().put(source, 0, words);
			staged.copyTo(commands, target, 0L, bytes);
		}
	}

	private void grow(int slots) {
		this.sections = Arrays.copyOf(this.sections, slots);
		this.isMarked = Arrays.copyOf(this.isMarked, slots);
		this.isFading = Arrays.copyOf(this.isFading, slots);
		this.instances = Arrays.copyOf(this.instances, slots * INSTANCE_WORDS);
		this.records = Arrays.copyOf(this.records, slots * RECORD_WORDS);
		this.vertexBuffers = Arrays.copyOf(this.vertexBuffers, slots * LAYERS);
		this.indexBuffers = Arrays.copyOf(this.indexBuffers, slots * LAYERS);
		this.wideIndices = Arrays.copyOf(this.wideIndices, slots * LAYERS);
		this.compactVertices = Arrays.copyOf(this.compactVertices, slots * LAYERS);
	}

	private static DeviceBuffer require(@Nullable DeviceBuffer buffer) {
		if (buffer == null) {
			throw new IllegalStateException("the section table was not flushed");
		}
		return buffer;
	}
}
