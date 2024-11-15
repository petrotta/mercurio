package io.github.petrotta.mercurio.plugins.type;

import io.github.petrotta.mercurio.plugins.PluginAnnotation;

public abstract class BasePlugin {
    protected PluginAnnotation getAnnotation() {
        return this.getClass().getAnnotation(PluginAnnotation.class);
    }
    public abstract void init();

}
