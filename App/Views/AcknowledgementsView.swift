import SwiftUI

/// Credits for the code and data Sevi is built from (menu: Ghi nhận…).
struct AcknowledgementsView: View {
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                section("Mã nguồn", "Sevi dùng mã nguồn của:", Acknowledgements.code)
                section("Dữ liệu từ ngữ", "Sevi tự chuyển Anh/Việt nhờ thống kê từ ngữ đếm từ các nguồn này. Trong ứng dụng chỉ có số liệu thống kê, không có văn bản nào.", Acknowledgements.wordData)
                section("Dữ liệu kiểm thử", "Chỉ dùng để đo và cải thiện độ chính xác, không có trong ứng dụng.", Acknowledgements.testData)
            }
            .padding(24)
        }
        .frame(width: 520, height: 600)
    }

    private func section(_ title: String, _ note: String, _ items: [Credit]) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(.headline)
            Text(note).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            ForEach(items) { c in
                HStack(alignment: .firstTextBaseline) {
                    VStack(alignment: .leading, spacing: 1) {
                        Link(c.name, destination: URL(string: c.url)!)
                        Text(c.by).font(.caption).foregroundStyle(.secondary)
                    }
                    Spacer()
                    Text(c.license).font(.caption).foregroundStyle(.secondary)
                }
            }
        }
    }
}
