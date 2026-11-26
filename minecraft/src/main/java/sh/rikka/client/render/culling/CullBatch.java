package sh.rikka.client.render.culling;

import java.util.Arrays;

final class CullBatch {
	private static final int INITIAL_CAPACITY = 256;
	static final int BOX_VALUES = 6;

	double cameraX;
	double cameraY;
	double cameraZ;
	int stamp;
	int size;
	CullTicket[] tickets = new CullTicket[INITIAL_CAPACITY];
	int[] tags = new int[INITIAL_CAPACITY];
	double[] boxes = new double[INITIAL_CAPACITY * BOX_VALUES];

	void begin(double x, double y, double z, int frameStamp) {
		this.cameraX = x;
		this.cameraY = y;
		this.cameraZ = z;
		this.stamp = frameStamp;
		this.size = 0;
	}

	void add(CullTicket ticket, double minX, double minY, double minZ, double maxX, double maxY, double maxZ) {
		int index = this.size;
		if (index == this.tickets.length) {
			int capacity = index * 2;
			this.tickets = Arrays.copyOf(this.tickets, capacity);
			this.tags = Arrays.copyOf(this.tags, capacity);
			this.boxes = Arrays.copyOf(this.boxes, capacity * BOX_VALUES);
		}
		this.tickets[index] = ticket;
		this.tags[index] = ticket.epoch;
		int box = index * BOX_VALUES;
		this.boxes[box] = minX;
		this.boxes[box + 1] = minY;
		this.boxes[box + 2] = minZ;
		this.boxes[box + 3] = maxX;
		this.boxes[box + 4] = maxY;
		this.boxes[box + 5] = maxZ;
		this.size = index + 1;
	}
}
