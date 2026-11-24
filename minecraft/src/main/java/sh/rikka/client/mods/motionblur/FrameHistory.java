package sh.rikka.client.mods.motionblur;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.pipeline.TextureTarget;
import sh.rikka.client.render.post.ScreenBuffer;
import sh.rikka.client.render.post.ScreenPass;

final class FrameHistory {
	private static final int SLOTS = MotionBlurPipelines.MAX_BLENDED_FRAMES;
	private static final String LABEL = "Motion blur frame";

	private final Frame[] ring = new Frame[SLOTS];
	private int head;
	private int count;

	int size() {
		return this.count;
	}

	double time(int index) {
		return frame(index).time;
	}

	RenderTarget target(int index) {
		return frame(index).target;
	}

	void record(RenderTarget source, double time, double minInterval, int capacity) {
		if (this.count > 0 && !ScreenBuffer.matches(frame(0).target, source)) {
			clear();
		}
		trim(capacity);
		Frame frame = slotFor(source, time, minInterval, capacity);
		ScreenPass.copy(source, frame.target);
		frame.time = time;
	}

	void clear() {
		trim(0);
		this.head = 0;
	}

	private Frame slotFor(RenderTarget source, double time, double minInterval, int capacity) {
		if (this.count >= 2 && time - time(this.count - 2) < minInterval) {
			return frame(this.count - 1);
		}
		if (this.count < capacity) {
			Frame frame = new Frame(ScreenBuffer.allocate(LABEL, source));
			this.ring[(this.head + this.count) % SLOTS] = frame;
			this.count++;
			return frame;
		}
		Frame oldest = this.ring[this.head];
		this.ring[this.head] = null;
		this.ring[(this.head + this.count) % SLOTS] = oldest;
		this.head = (this.head + 1) % SLOTS;
		return oldest;
	}

	private void trim(int capacity) {
		while (this.count > capacity) {
			this.ring[this.head].target.destroyBuffers();
			this.ring[this.head] = null;
			this.head = (this.head + 1) % SLOTS;
			this.count--;
		}
	}

	private Frame frame(int index) {
		return this.ring[(this.head + index) % SLOTS];
	}

	private static final class Frame {
		private final TextureTarget target;
		private double time;

		private Frame(TextureTarget target) {
			this.target = target;
		}
	}
}
