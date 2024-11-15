package io.github.petrotta.mercurio.plugins.type;

import io.github.petrotta.mercurio.build.Project;
import io.github.petrotta.mercurio.plugins.PluginAnnotation;

public abstract class TaskPlugin extends BasePlugin {
    public abstract void init();
    public abstract void run(Project project);



}
