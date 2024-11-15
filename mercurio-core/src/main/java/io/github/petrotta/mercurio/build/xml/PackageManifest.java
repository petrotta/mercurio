package io.github.petrotta.mercurio.build.xml;

import com.fasterxml.jackson.annotation.JsonPropertyOrder;
import com.fasterxml.jackson.annotation.JsonSetter;
import com.fasterxml.jackson.annotation.Nulls;
import com.fasterxml.jackson.dataformat.xml.annotation.JacksonXmlElementWrapper;
import com.fasterxml.jackson.dataformat.xml.annotation.JacksonXmlProperty;
import com.fasterxml.jackson.dataformat.xml.annotation.JacksonXmlRootElement;
import lombok.Getter;
import lombok.Setter;

import java.util.ArrayList;
import java.util.List;


@JsonPropertyOrder({ "org", "project", "version", "description" })
@JacksonXmlRootElement( localName = "package")
public class PackageManifest {

//    @JacksonXmlElementWrapper(localName = "dependencies" )
//    @JsonSetter(nulls = Nulls.AS_EMPTY)
    @JacksonXmlProperty(localName = "dependencies")
//    @JacksonXmlElementWrapper(useWrapping = false)
    @Getter private List<Coordinate> dependencies = new ArrayList<>();

    @Getter @Setter private String org;
    @Getter @Setter private String description;
    @Getter @Setter private String project;
    @Getter @Setter private String version;

}