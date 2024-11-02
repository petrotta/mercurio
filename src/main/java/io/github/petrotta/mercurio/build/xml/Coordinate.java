package io.github.petrotta.mercurio.build.xml;

import com.fasterxml.jackson.dataformat.xml.annotation.JacksonXmlRootElement;
import lombok.Getter;
import lombok.Setter;

@JacksonXmlRootElement( localName = "coordinate")
public class Coordinate {
    @Getter @Setter private String org, version, project;
    @Getter @Setter private Repo repo;
}
