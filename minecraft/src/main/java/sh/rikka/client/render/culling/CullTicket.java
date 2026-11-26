package sh.rikka.client.render.culling;

import org.jspecify.annotations.Nullable;

public final class CullTicket {
	private static final long TAG_SHIFT = 32;
	private static final long STAMP_MASK = 0xFFFF_FFFFL;

	@Nullable CullingSession session;
	int epoch;
	int cameraEpoch;
	long recordedFrame = Long.MIN_VALUE;
	long leashedFrame = Long.MIN_VALUE;
	double x;
	double y;
	double z;
	final double[] testedBox = new double[CullBatch.BOX_VALUES];
	long testedWorker;
	long testedGrid;
	long testedCamera;
	boolean testedVisible;
	private volatile long result;

	void restart(CullingSession owner, int ownerCameraEpoch) {
		this.session = owner;
		this.cameraEpoch = ownerCameraEpoch;
		this.epoch++;
	}

	boolean isCulled(int stamp, int holdMillis) {
		long current = this.result;
		int since = (int) current;
		return (int) (current >>> TAG_SHIFT) == this.epoch && since != 0 && stamp - since >= holdMillis;
	}

	void publish(int tag, boolean visible, int stamp) {
		long tagBits = (long) tag << TAG_SHIFT;
		if (visible) {
			this.result = tagBits;
			return;
		}
		long previous = this.result;
		int since = (int) (previous >>> TAG_SHIFT) == tag && (int) previous != 0 ? (int) previous : stamp;
		this.result = tagBits | (since & STAMP_MASK);
	}
}
