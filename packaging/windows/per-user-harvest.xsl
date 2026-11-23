<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:wix="http://schemas.microsoft.com/wix/2006/wi" exclude-result-prefixes="wix">
    <xsl:output method="xml" indent="yes" />
    <xsl:template match="@*|node()">
        <xsl:copy><xsl:apply-templates select="@*|node()" /></xsl:copy>
    </xsl:template>

    <!-- ICE38/ICE64: files in a user profile need HKCU key paths and their folders need uninstall rows. -->
    <xsl:template match="wix:File/@KeyPath" />
    <xsl:template match="wix:Component">
        <xsl:copy>
            <xsl:apply-templates select="@*|node()" />
            <RegistryValue xmlns="http://schemas.microsoft.com/wix/2006/wi"
                Root="HKCU" Key="Software\GoldCoders\eBIRFormsMSI\Components"
                Name="{@Id}" Value="1" Type="integer" KeyPath="yes" />
            <xsl:if test="parent::wix:DirectoryRef and not(preceding-sibling::wix:Component)">
                <xsl:for-each select="//wix:Directory">
                    <RemoveFolder xmlns="http://schemas.microsoft.com/wix/2006/wi" Id="{concat('rm', @Id)}" Directory="{@Id}" On="uninstall" />
                </xsl:for-each>
                <RemoveFolder xmlns="http://schemas.microsoft.com/wix/2006/wi" Id="RemoveNshInstallDir" On="uninstall" />
            </xsl:if>
        </xsl:copy>
    </xsl:template>
</xsl:stylesheet>
