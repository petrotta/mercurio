package io.github.petrotta.mercurio.plugins.type;

import io.github.petrotta.mercurio.plugins.PluginAnnotation;
import lombok.Getter;
import lombok.Setter;

public abstract class LanguagePlugin extends BasePlugin {

    class LanguageDescription {
        @Getter @Setter public String[] fileTypes;
        @Getter @Setter public String name;
        @Getter @Setter public String description;
    }

    public abstract LanguageDescription getLanguageDescription();


}
