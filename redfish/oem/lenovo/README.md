# Lenovo OEM extensions

In this directory Lenovo-related features expect to see CSDL schemas.

If you have lenovo feature + accounts feature you need LenovoAccountService_v1.xml
which can be downloaded from your Lenovo server BMC.

```
curl -fLO "https://{lenovo-bmc-ip}/redfish/v1/metadata/LenovoAccountService_v1.xml"
```
