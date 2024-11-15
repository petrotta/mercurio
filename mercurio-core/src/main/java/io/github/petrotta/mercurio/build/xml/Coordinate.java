package io.github.petrotta.mercurio.build.xml;

import com.fasterxml.jackson.annotation.JsonSubTypes;
import com.fasterxml.jackson.annotation.JsonTypeInfo;
import com.fasterxml.jackson.dataformat.xml.annotation.JacksonXmlProperty;
import com.fasterxml.jackson.dataformat.xml.annotation.JacksonXmlRootElement;
import lombok.Getter;
import lombok.Setter;

@JacksonXmlRootElement( localName = "coordinate")
public class Coordinate {

    @Getter @Setter private String org, version, project;
    @Getter @Setter private Repo repo;

}
