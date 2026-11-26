package sh.rikka.bench;

import java.io.IOException;

@FunctionalInterface
interface Step {
	boolean advance() throws IOException;
}
