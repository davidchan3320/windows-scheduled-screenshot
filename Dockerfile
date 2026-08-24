FROM mcr.microsoft.com/dotnet/sdk:10.0@sha256:0e53453ccfc8ff2d51319fe80c678971c6d0f8008dff3565fa88e15840b69854 AS restore
RUN groupadd --gid 10001 builder && useradd --uid 10001 --gid builder --create-home builder
WORKDIR /workspace
RUN chown builder:builder /workspace
USER builder
COPY --chown=builder:builder ScreenCapture.sln ./
COPY --chown=builder:builder src/ScheduledScreenshot/ScheduledScreenshot.csproj src/ScheduledScreenshot/
COPY --chown=builder:builder tests/ScheduledScreenshot.Tests/ScheduledScreenshot.Tests.csproj tests/ScheduledScreenshot.Tests/
RUN dotnet restore ScreenCapture.sln

FROM restore AS build
COPY --chown=builder:builder . .
RUN dotnet build ScreenCapture.sln --configuration Release --no-restore --property:Platform=x64

FROM build AS test-build
RUN dotnet test tests/ScheduledScreenshot.Tests/ScheduledScreenshot.Tests.csproj --configuration Release --no-build --property:Platform=x64

FROM scratch AS artifacts
COPY --from=test-build /workspace/src/ScheduledScreenshot/bin/x64/Release/net48/ /
