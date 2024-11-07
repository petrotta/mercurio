package io.github.petrotta.mercurio.plugins;

import com.fasterxml.jackson.annotation.JsonSetter;
import com.fasterxml.jackson.annotation.Nulls;
import com.fasterxml.jackson.dataformat.xml.annotation.JacksonXmlElementWrapper;
import com.fasterxml.jackson.dataformat.xml.annotation.JacksonXmlProperty;
import com.fasterxml.jackson.dataformat.xml.annotation.JacksonXmlRootElement;
import io.github.petrotta.mercurio.build.xml.Coordinate;
import lombok.Getter;
import lombok.Setter;

import java.util.ArrayList;
import java.util.List;

@JacksonXmlRootElement( localName = "plugin")
public class PluginXML {

    @JacksonXmlProperty(localName = "entry-class" )
    @Getter @Setter private String entryClass;
}
