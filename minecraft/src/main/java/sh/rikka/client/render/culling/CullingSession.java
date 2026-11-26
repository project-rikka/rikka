package sh.rikka.client.render.culling;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.atomic.AtomicReference;
import java.util.concurrent.locks.LockSupport;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import org.jspecify.annotations.Nullable;

final class CullingSession {
	private static final String THREAD_NAME = "Rikka Entity Culling";
	private static final int HOLD_MILLIS = 200;
	private static final double JUMP_DISTANCE = 2.0;
	private static final long NANOS_PER_MILLI = 1_000_000L;
	private static final int MAX_CHUNK_RING = 128;

	final ClientLevel level;
	private final long startNanos = System.nanoTime();
	private final ConcurrentLinkedQueue<GridUpdate> updates = new ConcurrentLinkedQueue<>();
	private final AtomicReference<@Nullable CullBatch> pending = new AtomicReference<>();
	private final AtomicReference<@Nullable CullBatch> spare = new AtomicReference<>(new CullBatch());
	private final AtomicReference<@Nullable CullBatch> secondSpare = new AtomicReference<>(new CullBatch());
	private final AtomicReference<@Nullable Throwable> failure = new AtomicReference<>();
	private final Thread thread;
	private volatile boolean closed;
	private CullBatch filling = new CullBatch();
	private long frame;
	private int stamp;
	private int cameraEpoch;
	private double cameraX = Double.NaN;
	private double cameraY = Double.NaN;
	private double cameraZ = Double.NaN;

	CullingSession(ClientLevel level, double cameraX, double cameraY, double cameraZ) {
		this.level = level;
		OcclusionGrid grid = new OcclusionGrid(level.getMinSectionY(), level.getMaxSectionY());
		this.queueLoadedChunks(ChunkPos.containing(BlockPos.containing(cameraX, cameraY, cameraZ)));
		this.thread = Thread.ofPlatform().daemon().name(THREAD_NAME).start(() -> this.run(grid));
	}

	void chunkLoaded(LevelChunk chunk) {
		LevelChunkSection[] sections = chunk.getSections();
		List<GridUpdate.SectionStates> copies = new ArrayList<>();
		for (int index = 0; index < sections.length; index++) {
			LevelChunkSection section = sections[index];
			if (!section.hasOnlyAir() && section.maybeHas(BlockState::isSolidRender)) {
				copies.add(new GridUpdate.SectionStates(chunk.getSectionYFromSectionIndex(index), section.getStates().copy()));
			}
		}
		ChunkPos pos = chunk.getPos();
		this.updates.add(new GridUpdate.ChunkLoaded(pos.x(), pos.z(), List.copyOf(copies)));
	}

	void chunkUnloaded(int chunkX, int chunkZ) {
		this.updates.add(new GridUpdate.ChunkUnloaded(chunkX, chunkZ));
	}

	void blockChanged(int x, int y, int z, boolean opaque) {
		this.updates.add(new GridUpdate.BlockChanged(x, y, z, opaque));
	}

	void beginFrame(double x, double y, double z) {
		Throwable failed = this.failure.get();
		if (failed != null) {
			throw new IllegalStateException("entity culling failed", failed);
		}
		double dx = x - this.cameraX;
		double dy = y - this.cameraY;
		double dz = z - this.cameraZ;
		if (!(dx * dx + dy * dy + dz * dz <= JUMP_DISTANCE * JUMP_DISTANCE)) {
			this.cameraEpoch++;
		}
		this.cameraX = x;
		this.cameraY = y;
		this.cameraZ = z;
		this.frame++;
		int now = (int) ((System.nanoTime() - this.startNanos) / NANOS_PER_MILLI);
		this.stamp = now == 0 ? 1 : now;
		this.filling.begin(x, y, z, this.stamp);
	}

	boolean refresh(CullTicket ticket, double minX, double minY, double minZ) {
		double dx = minX - ticket.x;
		double dy = minY - ticket.y;
		double dz = minZ - ticket.z;
		if (ticket.session != this
			|| ticket.cameraEpoch != this.cameraEpoch
			|| ticket.recordedFrame != this.frame - 1
			|| !(dx * dx + dy * dy + dz * dz <= JUMP_DISTANCE * JUMP_DISTANCE)) {
			ticket.restart(this, this.cameraEpoch);
			return false;
		}
		return ticket.isCulled(this.stamp, HOLD_MILLIS);
	}

	void record(CullTicket ticket, double minX, double minY, double minZ, double maxX, double maxY, double maxZ) {
		ticket.recordedFrame = this.frame;
		ticket.x = minX;
		ticket.y = minY;
		ticket.z = minZ;
		this.filling.add(ticket, minX, minY, minZ, maxX, maxY, maxZ);
	}

	void markLeashed(CullTicket ticket) {
		ticket.leashedFrame = this.frame;
	}

	boolean isLeashed(CullTicket ticket) {
		return ticket.leashedFrame >= this.frame - 1;
	}

	double cameraX() {
		return this.cameraX;
	}

	double cameraY() {
		return this.cameraY;
	}

	double cameraZ() {
		return this.cameraZ;
	}

	void publish() {
		CullBatch published = this.filling;
		CullBatch previous = this.pending.getAndSet(published);
		this.filling = previous != null ? previous : this.takeSpare();
		LockSupport.unpark(this.thread);
	}

	void close() {
		this.closed = true;
		LockSupport.unpark(this.thread);
	}

	private CullBatch takeSpare() {
		CullBatch batch = this.spare.getAndSet(null);
		if (batch == null) {
			batch = this.secondSpare.getAndSet(null);
		}
		if (batch == null) {
			throw new IllegalStateException("entity culling lost a batch");
		}
		return batch;
	}

	private void giveBack(CullBatch batch) {
		if (!this.spare.compareAndSet(null, batch)) {
			this.secondSpare.set(batch);
		}
	}

	private void queueLoadedChunks(ChunkPos center) {
		int expected = this.level.getChunkSource().getLoadedChunksCount();
		int found = this.queueChunk(center.x(), center.z());
		for (int ring = 1; ring <= MAX_CHUNK_RING && found < expected; ring++) {
			for (int offset = -ring; offset <= ring; offset++) {
				found += this.queueChunk(center.x() + offset, center.z() - ring);
				found += this.queueChunk(center.x() + offset, center.z() + ring);
			}
			for (int offset = 1 - ring; offset < ring; offset++) {
				found += this.queueChunk(center.x() - ring, center.z() + offset);
				found += this.queueChunk(center.x() + ring, center.z() + offset);
			}
		}
	}

	private int queueChunk(int chunkX, int chunkZ) {
		LevelChunk chunk = this.level.getChunkSource().getChunk(chunkX, chunkZ, ChunkStatus.FULL, false);
		if (chunk == null) {
			return 0;
		}
		this.chunkLoaded(chunk);
		return 1;
	}

	private void run(OcclusionGrid grid) {
		CullingWorker worker = new CullingWorker(grid);
		try {
			while (!this.closed) {
				CullBatch batch = this.pending.getAndSet(null);
				if (batch == null) {
					LockSupport.park(this);
					continue;
				}
				GridUpdate update;
				while ((update = this.updates.poll()) != null) {
					worker.apply(update);
				}
				worker.test(batch);
				this.giveBack(batch);
			}
		} catch (Throwable error) {
			this.failure.set(error);
		}
	}
}
