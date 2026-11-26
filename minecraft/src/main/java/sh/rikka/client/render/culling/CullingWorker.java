package sh.rikka.client.render.culling;

import java.util.concurrent.atomic.AtomicLong;

final class CullingWorker {
	private static final double SAMPLE_SPACING = 0.5;
	private static final double INSET = 1.0E-4;
	private static final int BOX_VALUES = CullBatch.BOX_VALUES;
	private static final AtomicLong IDENTITIES = new AtomicLong();

	private final OcclusionGrid grid;
	private final long identity = IDENTITIES.incrementAndGet();
	private long gridVersion;
	private long cameraVersion;
	private int cameraBlockX;
	private int cameraBlockY;
	private int cameraBlockZ;
	private double cameraX = Double.NaN;
	private double cameraY = Double.NaN;
	private double cameraZ = Double.NaN;

	CullingWorker(OcclusionGrid grid) {
		this.grid = grid;
	}

	void apply(GridUpdate update) {
		this.grid.apply(update);
		this.gridVersion++;
	}

	void test(CullBatch batch) {
		if (batch.cameraX != this.cameraX || batch.cameraY != this.cameraY || batch.cameraZ != this.cameraZ) {
			this.cameraX = batch.cameraX;
			this.cameraY = batch.cameraY;
			this.cameraZ = batch.cameraZ;
			this.cameraBlockX = floor(batch.cameraX);
			this.cameraBlockY = floor(batch.cameraY);
			this.cameraBlockZ = floor(batch.cameraZ);
			this.cameraVersion++;
		}
		boolean cameraInsideBlock = this.grid.isOpaque(this.cameraBlockX, this.cameraBlockY, this.cameraBlockZ);
		double[] boxes = batch.boxes;
		for (int i = 0; i < batch.size; i++) {
			CullTicket ticket = batch.tickets[i];
			int box = i * BOX_VALUES;
			if (!this.isTested(ticket, boxes, box)) {
				ticket.testedVisible = cameraInsideBlock
					|| this.isBoxVisible(boxes[box], boxes[box + 1], boxes[box + 2], boxes[box + 3], boxes[box + 4], boxes[box + 5]);
				ticket.testedWorker = this.identity;
				ticket.testedGrid = this.gridVersion;
				ticket.testedCamera = this.cameraVersion;
				System.arraycopy(boxes, box, ticket.testedBox, 0, BOX_VALUES);
			}
			ticket.publish(batch.tags[i], ticket.testedVisible, batch.stamp);
		}
	}

	private boolean isTested(CullTicket ticket, double[] boxes, int box) {
		if (ticket.testedWorker != this.identity || ticket.testedGrid != this.gridVersion || ticket.testedCamera != this.cameraVersion) {
			return false;
		}
		double[] tested = ticket.testedBox;
		for (int value = 0; value < BOX_VALUES; value++) {
			if (tested[value] != boxes[box + value]) {
				return false;
			}
		}
		return true;
	}

	private boolean isBoxVisible(double minX, double minY, double minZ, double maxX, double maxY, double maxZ) {
		double x = this.cameraX;
		double y = this.cameraY;
		double z = this.cameraZ;
		boolean belowX = x < minX;
		boolean aboveX = x > maxX;
		boolean belowY = y < minY;
		boolean aboveY = y > maxY;
		boolean belowZ = z < minZ;
		boolean aboveZ = z > maxZ;
		if (!(belowX || aboveX || belowY || aboveY || belowZ || aboveZ)) {
			return true;
		}
		return belowX && this.isFaceXVisible(minX + INSET, minY, minZ, maxY, maxZ)
			|| aboveX && this.isFaceXVisible(maxX - INSET, minY, minZ, maxY, maxZ)
			|| belowY && this.isFaceYVisible(minY + INSET, minX, minZ, maxX, maxZ)
			|| aboveY && this.isFaceYVisible(maxY - INSET, minX, minZ, maxX, maxZ)
			|| belowZ && this.isFaceZVisible(minZ + INSET, minX, minY, maxX, maxY)
			|| aboveZ && this.isFaceZVisible(maxZ - INSET, minX, minY, maxX, maxY);
	}

	private boolean isFaceXVisible(double x, double minY, double minZ, double maxY, double maxZ) {
		int stepsY = steps(maxY - minY);
		int stepsZ = steps(maxZ - minZ);
		for (int j = 0; j <= stepsY; j++) {
			double y = sample(minY, maxY, j, stepsY);
			for (int k = 0; k <= stepsZ; k++) {
				if (this.isPointVisible(x, y, sample(minZ, maxZ, k, stepsZ))) {
					return true;
				}
			}
		}
		return false;
	}

	private boolean isFaceYVisible(double y, double minX, double minZ, double maxX, double maxZ) {
		int stepsX = steps(maxX - minX);
		int stepsZ = steps(maxZ - minZ);
		for (int i = 0; i <= stepsX; i++) {
			double x = sample(minX, maxX, i, stepsX);
			for (int k = 0; k <= stepsZ; k++) {
				if (this.isPointVisible(x, y, sample(minZ, maxZ, k, stepsZ))) {
					return true;
				}
			}
		}
		return false;
	}

	private boolean isFaceZVisible(double z, double minX, double minY, double maxX, double maxY) {
		int stepsX = steps(maxX - minX);
		int stepsY = steps(maxY - minY);
		for (int i = 0; i <= stepsX; i++) {
			double x = sample(minX, maxX, i, stepsX);
			for (int j = 0; j <= stepsY; j++) {
				if (this.isPointVisible(x, sample(minY, maxY, j, stepsY), z)) {
					return true;
				}
			}
		}
		return false;
	}

	private boolean isPointVisible(double startX, double startY, double startZ) {
		int x = floor(startX);
		int y = floor(startY);
		int z = floor(startZ);
		int targetX = this.cameraBlockX;
		int targetY = this.cameraBlockY;
		int targetZ = this.cameraBlockZ;
		double dx = this.cameraX - startX;
		double dy = this.cameraY - startY;
		double dz = this.cameraZ - startZ;
		int stepX = Double.compare(dx, 0.0);
		int stepY = Double.compare(dy, 0.0);
		int stepZ = Double.compare(dz, 0.0);
		double deltaX = stepX == 0 ? Double.POSITIVE_INFINITY : Math.abs(1.0 / dx);
		double deltaY = stepY == 0 ? Double.POSITIVE_INFINITY : Math.abs(1.0 / dy);
		double deltaZ = stepZ == 0 ? Double.POSITIVE_INFINITY : Math.abs(1.0 / dz);
		double nextX = stepX == 0 ? Double.POSITIVE_INFINITY : (stepX > 0 ? x + 1 - startX : startX - x) * deltaX;
		double nextY = stepY == 0 ? Double.POSITIVE_INFINITY : (stepY > 0 ? y + 1 - startY : startY - y) * deltaY;
		double nextZ = stepZ == 0 ? Double.POSITIVE_INFINITY : (stepZ > 0 ? z + 1 - startZ : startZ - z) * deltaZ;
		int remaining = Math.abs(targetX - x) + Math.abs(targetY - y) + Math.abs(targetZ - z);
		while (remaining-- > 0) {
			if (this.grid.isOpaque(x, y, z)) {
				return false;
			}
			if (nextX <= nextY && nextX <= nextZ) {
				if (nextX > 1.0) {
					return true;
				}
				x += stepX;
				nextX += deltaX;
			} else if (nextY <= nextZ) {
				if (nextY > 1.0) {
					return true;
				}
				y += stepY;
				nextY += deltaY;
			} else {
				if (nextZ > 1.0) {
					return true;
				}
				z += stepZ;
				nextZ += deltaZ;
			}
		}
		return true;
	}

	private static int steps(double extent) {
		return Math.max(1, (int) Math.ceil(extent / SAMPLE_SPACING));
	}

	private static double sample(double min, double max, int step, int steps) {
		return min + INSET + (max - min - 2.0 * INSET) * step / steps;
	}

	private static int floor(double value) {
		return (int) Math.floor(value);
	}
}
