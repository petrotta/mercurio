package io.github.petrotta.mercurio.build.xml;

import com.fasterxml.jackson.dataformat.xml.annotation.JacksonXmlElementWrapper;
import com.fasterxml.jackson.dataformat.xml.annotation.JacksonXmlRootElement;
import lombok.Getter;
import lombok.Setter;

import java.util.ArrayList;
import java.util.List;

@JacksonXmlRootElement( localName = "package")
public class PackageManifest {

    @JacksonXmlElementWrapper(localName = "dependencies")
    @Getter private List<Coordinate> dependencies = new ArrayList<>();
    @Getter @Setter private String org;
    @Getter @Setter private String description;
    @Getter @Setter private String project;
    @Getter @Setter private String version;

}