package sh.rikka.client.resource.legacy;

import java.io.IOException;

@FunctionalInterface
interface Upgrade {
	void apply(Conversion conversion) throws IOException;
}
